use crate::{
    discord_credentials::DiscordCredentials,
    social::{NativeSnapshot, SocialFriend, SocialIdentity},
};
use std::{
    ffi::{c_void, CString},
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Debug)]
pub(crate) enum NativeCommand {
    Connect,
    Disconnect,
    Invite { friend_id: String, content: String },
    PublishActivity {
        game_title: String,
        lobby_id: String,
        join_secret: String,
        party_size: u32,
        party_capacity: u32,
    },
    ClearActivity,
}

#[derive(Clone)]
struct NativeActivity {
    game_title: String,
    lobby_id: String,
    join_secret: String,
    party_size: u32,
    party_capacity: u32,
}

pub(crate) struct NativeRuntime {
    commands: mpsc::Sender<NativeCommand>,
}

impl NativeRuntime {
    pub(crate) fn start(
        application_id: u64,
        snapshot: Arc<Mutex<NativeSnapshot>>,
        emit: Arc<dyn Fn() + Send + Sync>,
        saved_credentials: Option<DiscordCredentials>,
        remember_credentials: Arc<dyn Fn(DiscordCredentials) + Send + Sync>,
        clear_credentials: Arc<dyn Fn() + Send + Sync>,
        on_join: Arc<dyn Fn(String) + Send + Sync>,
    ) -> Result<Self, String> {
        let (commands, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("coldem-discord-social".into())
            .spawn(move || {
                run(
                    application_id,
                    snapshot,
                    emit,
                    receiver,
                    saved_credentials,
                    remember_credentials,
                    clear_credentials,
                    on_join,
                )
            })
            .map_err(|error| format!("Could not start Discord Social SDK: {error}"))?;
        Ok(Self { commands })
    }

    pub(crate) fn send(&self, command: NativeCommand) -> Result<(), String> {
        self.commands
            .send(command)
            .map_err(|_| "The Discord Social SDK runtime stopped.".to_string())
    }
}

struct CallbackState {
    client: usize,
    application_id: u64,
    snapshot: Arc<Mutex<NativeSnapshot>>,
    emit: Arc<dyn Fn() + Send + Sync>,
    verifier: Option<DiscordAuthorizationCodeVerifier>,
    pending_credentials: Option<DiscordCredentials>,
    restoring_session: bool,
    remember_credentials: Arc<dyn Fn(DiscordCredentials) + Send + Sync>,
    clear_credentials: Arc<dyn Fn() + Send + Sync>,
    on_join: Arc<dyn Fn(String) + Send + Sync>,
    pending_activity: Option<NativeActivity>,
}

fn run(
    application_id: u64,
    snapshot: Arc<Mutex<NativeSnapshot>>,
    emit: Arc<dyn Fn() + Send + Sync>,
    receiver: mpsc::Receiver<NativeCommand>,
    saved_credentials: Option<DiscordCredentials>,
    remember_credentials: Arc<dyn Fn(DiscordCredentials) + Send + Sync>,
    clear_credentials: Arc<dyn Fn() + Send + Sync>,
    on_join: Arc<dyn Fn(String) + Send + Sync>,
) {
    let mut client = DiscordClient {
        opaque: std::ptr::null_mut(),
    };
    unsafe {
        Discord_Client_Init(&mut client);
        Discord_Client_SetApplicationId(&mut client, application_id);
    }
    let mut state = Box::new(CallbackState {
        client: (&mut client as *mut DiscordClient) as usize,
        application_id,
        snapshot,
        emit,
        verifier: None,
        pending_credentials: None,
        restoring_session: saved_credentials.is_some(),
        remember_credentials,
        clear_credentials,
        on_join,
        pending_activity: None,
    });
    let state_ptr = (&mut *state) as *mut CallbackState as *mut c_void;
    unsafe {
        Discord_Client_SetStatusChangedCallback(&mut client, status_callback, None, state_ptr);
        Discord_Client_SetRelationshipGroupsUpdatedCallback(
            &mut client,
            relationship_groups_callback,
            None,
            state_ptr,
        );
        Discord_Client_SetUserUpdatedCallback(&mut client, user_updated_callback, None, state_ptr);
        Discord_Client_SetActivityJoinCallback(&mut client, activity_join_callback, None, state_ptr);
        let launch = CString::new("coldem://discord/join").expect("static launch command");
        let _ = Discord_Client_RegisterLaunchCommand(
            &mut client,
            application_id,
            sdk_string(launch.as_bytes()),
        );
        if let Some(credentials) = saved_credentials {
            Discord_Client_RefreshToken(
                &mut client,
                application_id,
                sdk_string(credentials.refresh_token.as_bytes()),
                token_callback,
                None,
                state_ptr,
            );
        }
    }

    let mut last_refresh = Instant::now() - Duration::from_secs(2);
    loop {
        match receiver.recv_timeout(Duration::from_millis(40)) {
            Ok(command) => unsafe { handle_command(&mut state, command, state_ptr) },
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        unsafe {
            Discord_RunCallbacks();
        }
        if last_refresh.elapsed() >= Duration::from_secs(1)
            && unsafe { Discord_Client_GetStatus(&mut client) } == DiscordClientStatus::Ready as i32
        {
            unsafe {
                refresh_current_user(&state);
                refresh_friends(&state);
                if let Some(activity) = state.pending_activity.take() {
                    publish_activity(&mut state, activity, state_ptr);
                }
            }
            last_refresh = Instant::now();
        }
    }

    unsafe {
        if let Some(mut verifier) = state.verifier.take() {
            Discord_AuthorizationCodeVerifier_Drop(&mut verifier);
        }
        Discord_Client_Drop(&mut client);
    }
}

unsafe fn handle_command(
    state: &mut CallbackState,
    command: NativeCommand,
    state_ptr: *mut c_void,
) {
    let client = &mut *(state.client as *mut DiscordClient);
    match command {
        NativeCommand::Connect => {
            if Discord_Client_IsAuthenticated(client) {
                Discord_Client_Connect(client);
                return;
            }
            if state.verifier.is_some() || state.restoring_session {
                return;
            }
            let mut verifier = DiscordAuthorizationCodeVerifier {
                opaque: std::ptr::null_mut(),
            };
            Discord_Client_CreateAuthorizationCodeVerifier(client, &mut verifier);
            let mut challenge = DiscordAuthorizationCodeChallenge {
                opaque: std::ptr::null_mut(),
            };
            Discord_AuthorizationCodeVerifier_Challenge(&mut verifier, &mut challenge);
            let mut args = DiscordAuthorizationArgs {
                opaque: std::ptr::null_mut(),
            };
            Discord_AuthorizationArgs_Init(&mut args);
            Discord_AuthorizationArgs_SetClientId(&mut args, state.application_id);
            // Activity invites themselves work with the presence scopes, but the
            // optional text attached to an invite is a Discord message and needs
            // the communication scopes too. Ask the SDK for its supported
            // baseline rather than hard-coding a scope list that can drift.
            let mut scopes = DiscordString {
                ptr: std::ptr::null_mut(),
                size: 0,
            };
            Discord_Client_GetDefaultCommunicationScopes(&mut scopes);
            let scopes = CString::new(copy_sdk_string(scopes))
                .expect("Discord communication scopes contain no NUL bytes");
            Discord_AuthorizationArgs_SetScopes(&mut args, sdk_string(scopes.as_bytes()));
            Discord_AuthorizationArgs_SetCodeChallenge(&mut args, &mut challenge);
            state.verifier = Some(verifier);
            Discord_Client_Authorize(client, &mut args, authorization_callback, None, state_ptr);
            Discord_AuthorizationArgs_Drop(&mut args);
            Discord_AuthorizationCodeChallenge_Drop(&mut challenge);
            set_message(state_ptr, "Complete Discord authorization in your browser.");
        }
        NativeCommand::Disconnect => Discord_Client_Disconnect(client),
        NativeCommand::Invite { friend_id, content } => {
            let Ok(friend_id) = friend_id.parse::<u64>() else {
                return;
            };
            let Ok(content) = CString::new(content) else {
                return;
            };
            Discord_Client_SendActivityInvite(
                client,
                friend_id,
                sdk_string(content.as_bytes()),
                invite_callback,
                None,
                state_ptr,
            );
        }
        NativeCommand::PublishActivity {
            game_title,
            lobby_id,
            join_secret,
            party_size,
            party_capacity,
        } => {
            let activity = NativeActivity {
                game_title,
                lobby_id,
                join_secret,
                party_size,
                party_capacity,
            };
            if Discord_Client_GetStatus(client) == DiscordClientStatus::Ready as i32 {
                publish_activity(state, activity, state_ptr);
            } else {
                state.pending_activity = Some(activity);
            }
        }
        NativeCommand::ClearActivity => {
            state.pending_activity = None;
            Discord_Client_ClearRichPresence(client);
        }
    }
}

unsafe fn publish_activity(
    state: &mut CallbackState,
    activity: NativeActivity,
    state_ptr: *mut c_void,
) {
    let Ok(name) = CString::new(activity.game_title.as_str()) else {
        set_message(state_ptr, "Could not publish Discord presence for this game title.");
        return;
    };
    let Ok(activity_state) = CString::new(format!(
        "{} · Hosting EOS lobby {}/{}",
        activity.game_title,
        activity.party_size, activity.party_capacity
    )) else {
        set_message(state_ptr, "Could not publish Discord lobby state.");
        return;
    };
    let details = CString::new("Discord invites open").expect("static activity details");
    let Ok(lobby_id) = CString::new(activity.lobby_id.as_str()) else {
        set_message(state_ptr, "Could not publish Discord presence for this EOS lobby.");
        return;
    };
    let Ok(join_secret) = CString::new(activity.join_secret.as_str()) else {
        set_message(state_ptr, "Could not publish the Discord join payload.");
        return;
    };

    let mut rich_presence = DiscordActivity {
        opaque: std::ptr::null_mut(),
    };
    let mut party = DiscordActivityParty {
        opaque: std::ptr::null_mut(),
    };
    let mut secrets = DiscordActivitySecrets {
        opaque: std::ptr::null_mut(),
    };
    Discord_Activity_Init(&mut rich_presence);
    Discord_ActivityParty_Init(&mut party);
    Discord_ActivitySecrets_Init(&mut secrets);
    Discord_Activity_SetType(&mut rich_presence, 0);
    Discord_Activity_SetName(&mut rich_presence, sdk_string(name.as_bytes()));
    let mut activity_state = sdk_string(activity_state.as_bytes());
    Discord_Activity_SetState(&mut rich_presence, &mut activity_state);
    let mut details = sdk_string(details.as_bytes());
    Discord_Activity_SetDetails(&mut rich_presence, &mut details);
    Discord_ActivityParty_SetId(&mut party, sdk_string(lobby_id.as_bytes()));
    Discord_ActivityParty_SetCurrentSize(&mut party, activity.party_size.min(i32::MAX as u32) as i32);
    Discord_ActivityParty_SetMaxSize(&mut party, activity.party_capacity.min(i32::MAX as u32) as i32);
    Discord_ActivityParty_SetPrivacy(&mut party, 1);
    Discord_Activity_SetParty(&mut rich_presence, &mut party);
    Discord_ActivitySecrets_SetJoin(&mut secrets, sdk_string(join_secret.as_bytes()));
    Discord_Activity_SetSecrets(&mut rich_presence, &mut secrets);
    Discord_Activity_SetSupportedPlatforms(&mut rich_presence, 1);
    Discord_Client_UpdateRichPresence(
        &mut *(state.client as *mut DiscordClient),
        &mut rich_presence,
        presence_callback,
        None,
        state_ptr,
    );
    Discord_ActivitySecrets_Drop(&mut secrets);
    Discord_ActivityParty_Drop(&mut party);
    Discord_Activity_Drop(&mut rich_presence);
}

unsafe extern "C" fn authorization_callback(
    result: *mut DiscordClientResult,
    code: DiscordString,
    redirect_uri: DiscordString,
    user_data: *mut c_void,
) {
    if result.is_null() || user_data.is_null() || !Discord_ClientResult_Successful(result) {
        set_message(user_data, "Discord authorization failed.");
        return;
    }
    let state = &mut *(user_data as *mut CallbackState);
    let Some(verifier) = state.verifier.as_ref() else {
        return;
    };
    let code = copy_sdk_string(code);
    let redirect_uri = copy_sdk_string(redirect_uri);
    let verifier = verifier_verifier(verifier);
    Discord_Client_GetToken(
        &mut *(state.client as *mut DiscordClient),
        state.application_id,
        sdk_string(code.as_bytes()),
        sdk_string(verifier.as_bytes()),
        sdk_string(redirect_uri.as_bytes()),
        token_callback,
        None,
        user_data,
    );
}

unsafe extern "C" fn token_callback(
    result: *mut DiscordClientResult,
    access_token: DiscordString,
    refresh_token: DiscordString,
    token_type: i32,
    _expires_in: i32,
    _scopes: DiscordString,
    user_data: *mut c_void,
) {
    if result.is_null() || user_data.is_null() || !Discord_ClientResult_Successful(result) {
        if !user_data.is_null() {
            let state = &mut *(user_data as *mut CallbackState);
            if state.restoring_session {
                (state.clear_credentials)();
                state.restoring_session = false;
                if let Ok(mut snapshot) = state.snapshot.lock() {
                    snapshot.connection = "disconnected".into();
                    snapshot.current_user = None;
                    snapshot.message = Some("Your saved Discord session expired. Connect again to restore it.".into());
                }
                (state.emit)();
                return;
            }
        }
        set_message(user_data, "Discord token exchange failed.");
        return;
    }
    let state = &mut *(user_data as *mut CallbackState);
    let credentials = DiscordCredentials {
        access_token: copy_sdk_string(access_token),
        refresh_token: copy_sdk_string(refresh_token),
        token_type,
    };
    if credentials.access_token.is_empty() || credentials.refresh_token.is_empty() {
        set_message(user_data, "Discord returned an incomplete session.");
        return;
    }
    state.pending_credentials = Some(credentials.clone());
    Discord_Client_UpdateToken(
        &mut *(state.client as *mut DiscordClient),
        credentials.token_type,
        sdk_string(credentials.access_token.as_bytes()),
        update_token_callback,
        None,
        user_data,
    );
}

unsafe extern "C" fn update_token_callback(
    result: *mut DiscordClientResult,
    user_data: *mut c_void,
) {
    if result.is_null() || user_data.is_null() || !Discord_ClientResult_Successful(result) {
        if !user_data.is_null() {
            let state = &mut *(user_data as *mut CallbackState);
            if state.restoring_session {
                (state.clear_credentials)();
                state.restoring_session = false;
                state.pending_credentials = None;
                if let Ok(mut snapshot) = state.snapshot.lock() {
                    snapshot.connection = "disconnected".into();
                    snapshot.current_user = None;
                    snapshot.message = Some("Your saved Discord session expired. Connect again to restore it.".into());
                }
                (state.emit)();
                return;
            }
        }
        set_message(user_data, "Discord token setup failed.");
        return;
    }
    let state = &mut *(user_data as *mut CallbackState);
    if let Some(credentials) = state.pending_credentials.take() {
        (state.remember_credentials)(credentials);
    }
    state.restoring_session = false;
    if let Some(mut verifier) = state.verifier.take() {
        Discord_AuthorizationCodeVerifier_Drop(&mut verifier);
    }
    Discord_Client_Connect(&mut *(state.client as *mut DiscordClient));
}

unsafe extern "C" fn status_callback(
    status: i32,
    _error: i32,
    _detail: i32,
    user_data: *mut c_void,
) {
    if user_data.is_null() {
        return;
    }
    let state = &mut *(user_data as *mut CallbackState);
    let (connection, message) = match status {
        3 => ("connected", "Discord friends are ready."),
        1 | 2 => ("connecting", "Connecting to Discord..."),
        _ => (
            "disconnected",
            "Connect Discord to see friends and activity invites.",
        ),
    };
    if let Ok(mut snapshot) = state.snapshot.lock() {
        snapshot.connection = connection.into();
        if connection != "connected" {
            snapshot.current_user = None;
        }
        snapshot.message = Some(message.into());
    }
    (state.emit)();
}

unsafe extern "C" fn relationship_groups_callback(_user_id: u64, user_data: *mut c_void) {
    if !user_data.is_null() {
        (&*(user_data as *mut CallbackState)).emit.as_ref()();
    }
}

unsafe extern "C" fn user_updated_callback(_user_id: u64, user_data: *mut c_void) {
    if !user_data.is_null() {
        (&*(user_data as *mut CallbackState)).emit.as_ref()();
    }
}

unsafe extern "C" fn invite_callback(_result: *mut DiscordClientResult, user_data: *mut c_void) {
    if !user_data.is_null() {
        set_message(user_data, "Discord invite sent.");
    }
}

unsafe extern "C" fn presence_callback(result: *mut DiscordClientResult, user_data: *mut c_void) {
    if result.is_null() || user_data.is_null() {
        return;
    }
    if Discord_ClientResult_Successful(result) {
        set_message(user_data, "Discord lobby invites are live.");
    } else {
        set_message(user_data, "Discord could not publish this lobby invite.");
    }
}

unsafe extern "C" fn activity_join_callback(join_secret: DiscordString, user_data: *mut c_void) {
    if user_data.is_null() {
        return;
    }
    let payload = copy_sdk_string(join_secret);
    if payload.is_empty() {
        set_message(user_data, "Discord sent an empty join payload.");
        return;
    }
    let state = &*(user_data as *mut CallbackState);
    (state.on_join)(payload);
}

unsafe fn refresh_friends(state: &CallbackState) {
    let mut friends = Vec::new();
    for (group, group_name) in [(0_i32, "playing"), (1_i32, "online"), (2_i32, "offline")] {
        let mut span = DiscordRelationshipHandleSpan {
            ptr: std::ptr::null_mut(),
            size: 0,
        };
        Discord_Client_GetRelationshipsByGroup(
            &mut *(state.client as *mut DiscordClient),
            group,
            &mut span,
        );
        for index in 0..span.size {
            let relationship = &mut *span.ptr.add(index);
            let mut user = DiscordUserHandle {
                opaque: std::ptr::null_mut(),
            };
            if Discord_RelationshipHandle_User(relationship, &mut user) {
                friends.push(SocialFriend {
                    id: Discord_RelationshipHandle_Id(relationship).to_string(),
                    display_name: user_string(&mut user, Discord_UserHandle_DisplayName),
                    group: group_name,
                    status_text: match group_name {
                        "playing" => "Playing Coldem",
                        "online" => "Online elsewhere",
                        _ => "Offline",
                    }
                    .into(),
                    avatar_url: Some(user_avatar_url(&mut user)),
                });
                Discord_UserHandle_Drop(&mut user);
            }
            Discord_RelationshipHandle_Drop(relationship);
        }
        if !span.ptr.is_null() {
            Discord_Free(span.ptr.cast());
        }
    }
    if let Ok(mut snapshot) = state.snapshot.lock() {
        snapshot.friends = friends;
        snapshot.connection = "connected".into();
    }
    (state.emit)();
}

unsafe fn refresh_current_user(state: &CallbackState) {
    let mut user = DiscordUserHandle {
        opaque: std::ptr::null_mut(),
    };
    if !Discord_Client_GetCurrentUserV2(&mut *(state.client as *mut DiscordClient), &mut user) {
        return;
    }

    let username = user_string(&mut user, Discord_UserHandle_Username);
    let avatar_url = user_avatar_url(&mut user);
    let identity = SocialIdentity {
        id: Discord_UserHandle_Id(&mut user).to_string(),
        display_name: user_string(&mut user, Discord_UserHandle_DisplayName),
        username: (!username.is_empty()).then_some(username),
        avatar_url: (!avatar_url.is_empty()).then_some(avatar_url),
    };
    Discord_UserHandle_Drop(&mut user);

    if let Ok(mut snapshot) = state.snapshot.lock() {
        snapshot.current_user = Some(identity);
    }
}

unsafe fn set_message(user_data: *mut c_void, message: &str) {
    if user_data.is_null() {
        return;
    }
    let state = &*(user_data as *mut CallbackState);
    if let Ok(mut snapshot) = state.snapshot.lock() {
        snapshot.message = Some(message.into());
    }
    (state.emit)();
}

unsafe fn user_string(
    user: &mut DiscordUserHandle,
    getter: unsafe extern "C" fn(*mut DiscordUserHandle, *mut DiscordString),
) -> String {
    let mut value = DiscordString {
        ptr: std::ptr::null_mut(),
        size: 0,
    };
    getter(user, &mut value);
    copy_sdk_string(value)
}

unsafe fn user_avatar_url(user: &mut DiscordUserHandle) -> String {
    let mut value = DiscordString {
        ptr: std::ptr::null_mut(),
        size: 0,
    };
    Discord_UserHandle_AvatarUrl(user, 1, 1, &mut value);
    copy_sdk_string(value)
}

unsafe fn verifier_verifier(verifier: &DiscordAuthorizationCodeVerifier) -> String {
    let mut value = DiscordString {
        ptr: std::ptr::null_mut(),
        size: 0,
    };
    Discord_AuthorizationCodeVerifier_Verifier(verifier as *const _ as *mut _, &mut value);
    copy_sdk_string(value)
}

unsafe fn copy_sdk_string(value: DiscordString) -> String {
    if value.ptr.is_null() {
        return String::new();
    }
    let text = std::slice::from_raw_parts(value.ptr, value.size);
    let result = String::from_utf8_lossy(text).into_owned();
    Discord_Free(value.ptr.cast());
    result
}

fn sdk_string(value: &[u8]) -> DiscordString {
    DiscordString {
        ptr: value.as_ptr() as *mut u8,
        size: value.len(),
    }
}

#[repr(C)]
struct DiscordString {
    ptr: *mut u8,
    size: usize,
}
#[repr(C)]
struct DiscordClient {
    opaque: *mut c_void,
}
#[repr(C)]
struct DiscordClientResult {
    opaque: *mut c_void,
}
#[repr(C)]
struct DiscordActivity {
    opaque: *mut c_void,
}
#[repr(C)]
struct DiscordActivityParty {
    opaque: *mut c_void,
}
#[repr(C)]
struct DiscordActivitySecrets {
    opaque: *mut c_void,
}
#[repr(C)]
struct DiscordAuthorizationArgs {
    opaque: *mut c_void,
}
#[repr(C)]
struct DiscordAuthorizationCodeVerifier {
    opaque: *mut c_void,
}
#[repr(C)]
struct DiscordAuthorizationCodeChallenge {
    opaque: *mut c_void,
}
#[repr(C)]
struct DiscordRelationshipHandle {
    opaque: *mut c_void,
}
#[repr(C)]
struct DiscordRelationshipHandleSpan {
    ptr: *mut DiscordRelationshipHandle,
    size: usize,
}
#[repr(C)]
struct DiscordUserHandle {
    opaque: *mut c_void,
}

#[allow(improper_ctypes)]
extern "C" {
    fn Discord_Free(ptr: *mut c_void);
    fn Discord_RunCallbacks();
    fn Discord_Client_Init(client: *mut DiscordClient);
    fn Discord_Client_Drop(client: *mut DiscordClient);
    fn Discord_Client_SetApplicationId(client: *mut DiscordClient, application_id: u64);
    fn Discord_Client_SetStatusChangedCallback(
        client: *mut DiscordClient,
        cb: unsafe extern "C" fn(i32, i32, i32, *mut c_void),
        free: Option<unsafe extern "C" fn(*mut c_void)>,
        user_data: *mut c_void,
    );
    fn Discord_Client_SetRelationshipGroupsUpdatedCallback(
        client: *mut DiscordClient,
        cb: unsafe extern "C" fn(u64, *mut c_void),
        free: Option<unsafe extern "C" fn(*mut c_void)>,
        user_data: *mut c_void,
    );
    fn Discord_Client_SetUserUpdatedCallback(
        client: *mut DiscordClient,
        cb: unsafe extern "C" fn(u64, *mut c_void),
        free: Option<unsafe extern "C" fn(*mut c_void)>,
        user_data: *mut c_void,
    );
    fn Discord_Client_GetStatus(client: *mut DiscordClient) -> i32;
    fn Discord_Client_GetCurrentUserV2(
        client: *mut DiscordClient,
        result: *mut DiscordUserHandle,
    ) -> bool;
    fn Discord_Client_IsAuthenticated(client: *mut DiscordClient) -> bool;
    fn Discord_Client_Connect(client: *mut DiscordClient);
    fn Discord_Client_Disconnect(client: *mut DiscordClient);
    fn Discord_Client_GetDefaultCommunicationScopes(return_value: *mut DiscordString);
    fn Discord_Client_CreateAuthorizationCodeVerifier(
        client: *mut DiscordClient,
        verifier: *mut DiscordAuthorizationCodeVerifier,
    );
    fn Discord_AuthorizationCodeVerifier_Challenge(
        verifier: *mut DiscordAuthorizationCodeVerifier,
        challenge: *mut DiscordAuthorizationCodeChallenge,
    );
    fn Discord_AuthorizationCodeVerifier_Verifier(
        verifier: *mut DiscordAuthorizationCodeVerifier,
        result: *mut DiscordString,
    );
    fn Discord_AuthorizationCodeVerifier_Drop(verifier: *mut DiscordAuthorizationCodeVerifier);
    fn Discord_AuthorizationArgs_Init(args: *mut DiscordAuthorizationArgs);
    fn Discord_AuthorizationArgs_Drop(args: *mut DiscordAuthorizationArgs);
    fn Discord_AuthorizationArgs_SetClientId(args: *mut DiscordAuthorizationArgs, client_id: u64);
    fn Discord_AuthorizationArgs_SetScopes(
        args: *mut DiscordAuthorizationArgs,
        scopes: DiscordString,
    );
    fn Discord_AuthorizationArgs_SetCodeChallenge(
        args: *mut DiscordAuthorizationArgs,
        challenge: *mut DiscordAuthorizationCodeChallenge,
    );
    fn Discord_AuthorizationCodeChallenge_Drop(challenge: *mut DiscordAuthorizationCodeChallenge);
    fn Discord_Client_Authorize(
        client: *mut DiscordClient,
        args: *mut DiscordAuthorizationArgs,
        cb: unsafe extern "C" fn(
            *mut DiscordClientResult,
            DiscordString,
            DiscordString,
            *mut c_void,
        ),
        free: Option<unsafe extern "C" fn(*mut c_void)>,
        user_data: *mut c_void,
    );
    fn Discord_ClientResult_Successful(result: *mut DiscordClientResult) -> bool;
    fn Discord_Client_GetToken(
        client: *mut DiscordClient,
        application_id: u64,
        code: DiscordString,
        verifier: DiscordString,
        redirect_uri: DiscordString,
        cb: unsafe extern "C" fn(
            *mut DiscordClientResult,
            DiscordString,
            DiscordString,
            i32,
            i32,
            DiscordString,
            *mut c_void,
        ),
        free: Option<unsafe extern "C" fn(*mut c_void)>,
        user_data: *mut c_void,
    );
    fn Discord_Client_RefreshToken(
        client: *mut DiscordClient,
        application_id: u64,
        refresh_token: DiscordString,
        cb: unsafe extern "C" fn(
            *mut DiscordClientResult,
            DiscordString,
            DiscordString,
            i32,
            i32,
            DiscordString,
            *mut c_void,
        ),
        free: Option<unsafe extern "C" fn(*mut c_void)>,
        user_data: *mut c_void,
    );
    fn Discord_Client_UpdateToken(
        client: *mut DiscordClient,
        token_type: i32,
        token: DiscordString,
        cb: unsafe extern "C" fn(*mut DiscordClientResult, *mut c_void),
        free: Option<unsafe extern "C" fn(*mut c_void)>,
        user_data: *mut c_void,
    );
    fn Discord_Client_SendActivityInvite(
        client: *mut DiscordClient,
        user_id: u64,
        content: DiscordString,
        cb: unsafe extern "C" fn(*mut DiscordClientResult, *mut c_void),
        free: Option<unsafe extern "C" fn(*mut c_void)>,
        user_data: *mut c_void,
    );
    fn Discord_Client_RegisterLaunchCommand(
        client: *mut DiscordClient,
        application_id: u64,
        command: DiscordString,
    ) -> bool;
    fn Discord_Client_SetActivityJoinCallback(
        client: *mut DiscordClient,
        cb: unsafe extern "C" fn(DiscordString, *mut c_void),
        free: Option<unsafe extern "C" fn(*mut c_void)>,
        user_data: *mut c_void,
    );
    fn Discord_Client_ClearRichPresence(client: *mut DiscordClient);
    fn Discord_Client_UpdateRichPresence(
        client: *mut DiscordClient,
        activity: *mut DiscordActivity,
        cb: unsafe extern "C" fn(*mut DiscordClientResult, *mut c_void),
        free: Option<unsafe extern "C" fn(*mut c_void)>,
        user_data: *mut c_void,
    );
    fn Discord_Activity_Init(activity: *mut DiscordActivity);
    fn Discord_Activity_Drop(activity: *mut DiscordActivity);
    fn Discord_Activity_SetType(activity: *mut DiscordActivity, value: i32);
    fn Discord_Activity_SetName(activity: *mut DiscordActivity, value: DiscordString);
    fn Discord_Activity_SetState(activity: *mut DiscordActivity, value: *mut DiscordString);
    fn Discord_Activity_SetDetails(activity: *mut DiscordActivity, value: *mut DiscordString);
    fn Discord_Activity_SetParty(activity: *mut DiscordActivity, value: *mut DiscordActivityParty);
    fn Discord_Activity_SetSecrets(activity: *mut DiscordActivity, value: *mut DiscordActivitySecrets);
    fn Discord_Activity_SetSupportedPlatforms(activity: *mut DiscordActivity, value: i32);
    fn Discord_ActivityParty_Init(party: *mut DiscordActivityParty);
    fn Discord_ActivityParty_Drop(party: *mut DiscordActivityParty);
    fn Discord_ActivityParty_SetId(party: *mut DiscordActivityParty, value: DiscordString);
    fn Discord_ActivityParty_SetCurrentSize(party: *mut DiscordActivityParty, value: i32);
    fn Discord_ActivityParty_SetMaxSize(party: *mut DiscordActivityParty, value: i32);
    fn Discord_ActivityParty_SetPrivacy(party: *mut DiscordActivityParty, value: i32);
    fn Discord_ActivitySecrets_Init(secrets: *mut DiscordActivitySecrets);
    fn Discord_ActivitySecrets_Drop(secrets: *mut DiscordActivitySecrets);
    fn Discord_ActivitySecrets_SetJoin(secrets: *mut DiscordActivitySecrets, value: DiscordString);
    fn Discord_Client_GetRelationshipsByGroup(
        client: *mut DiscordClient,
        group: i32,
        result: *mut DiscordRelationshipHandleSpan,
    );
    fn Discord_RelationshipHandle_User(
        relationship: *mut DiscordRelationshipHandle,
        result: *mut DiscordUserHandle,
    ) -> bool;
    fn Discord_RelationshipHandle_Id(relationship: *mut DiscordRelationshipHandle) -> u64;
    fn Discord_RelationshipHandle_Drop(relationship: *mut DiscordRelationshipHandle);
    fn Discord_UserHandle_Drop(user: *mut DiscordUserHandle);
    fn Discord_UserHandle_DisplayName(user: *mut DiscordUserHandle, result: *mut DiscordString);
    fn Discord_UserHandle_Username(user: *mut DiscordUserHandle, result: *mut DiscordString);
    fn Discord_UserHandle_Id(user: *mut DiscordUserHandle) -> u64;
    fn Discord_UserHandle_AvatarUrl(
        user: *mut DiscordUserHandle,
        animated: i32,
        static_type: i32,
        result: *mut DiscordString,
    );
}

#[repr(i32)]
enum DiscordClientStatus {
    Ready = 3,
}
