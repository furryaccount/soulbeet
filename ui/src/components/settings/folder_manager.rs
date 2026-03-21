use api::{create_user_folder, delete_folder, get_user_folders, update_folder};
use dioxus::prelude::*;

use crate::auth::use_auth;

#[component]
pub fn FolderManager() -> Element {
    let mut folder_name = use_signal(|| "".to_string());
    let mut folder_path = use_signal(|| "".to_string());
    let mut folders = use_signal(Vec::new);

    let mut editing_folder_id = use_signal(|| None::<String>);
    let mut edit_folder_name = use_signal(|| "".to_string());
    let mut edit_folder_path = use_signal(|| "".to_string());

    let mut error = use_signal(|| "".to_string());
    let mut success_msg = use_signal(|| "".to_string());
    let auth = use_auth();

    // Library settings state
    let mut auto_delete_enabled = use_signal(|| false);
    let mut discovery_promote_threshold = use_signal(|| "3".to_string());
    let mut lastfm_api_key = use_signal(String::new);
    let mut lastfm_username = use_signal(String::new);
    let mut lb_username = use_signal(String::new);
    let mut lb_token = use_signal(String::new);
    let mut settings_loaded = use_signal(|| false);

    // Discovery settings state
    let mut discovery_enabled = use_signal(|| false);
    let mut discovery_folder_id = use_signal(String::new);
    let mut discovery_track_count = use_signal(|| "20".to_string());
    let mut discovery_lifetime_days = use_signal(|| "7".to_string());
    let mut discovery_profiles = use_signal(|| "Conservative,Balanced,Adventurous".to_string());
    let mut pl_name_safe = use_signal(|| "Comfort Zone".to_string());
    let mut pl_name_mix = use_signal(|| "Fresh Picks".to_string());
    let mut pl_name_wild = use_signal(|| "Deep Cuts".to_string());

    use_future(move || async move {
        if let Ok(user_settings) = api::get_user_settings().await {
            auto_delete_enabled.set(user_settings.auto_delete_enabled);
            discovery_promote_threshold.set(user_settings.discovery_promote_threshold.to_string());
            lastfm_api_key.set(user_settings.lastfm_api_key.unwrap_or_default());
            lastfm_username.set(user_settings.lastfm_username.unwrap_or_default());
            lb_username.set(user_settings.listenbrainz_username.unwrap_or_default());
            lb_token.set(user_settings.listenbrainz_token.unwrap_or_default());
            discovery_enabled.set(user_settings.discovery_enabled);
            discovery_folder_id.set(user_settings.discovery_folder_id.unwrap_or_default());
            discovery_track_count.set(user_settings.discovery_track_count.to_string());
            discovery_lifetime_days.set(user_settings.discovery_lifetime_days.to_string());
            discovery_profiles.set(user_settings.discovery_profiles);
            // Parse per-profile playlist names from JSON
            if let Ok(names) = serde_json::from_str::<std::collections::HashMap<String, String>>(
                &user_settings.discovery_playlist_name,
            ) {
                if let Some(n) = names.get("Conservative") {
                    pl_name_safe.set(n.clone());
                }
                if let Some(n) = names.get("Balanced") {
                    pl_name_mix.set(n.clone());
                }
                if let Some(n) = names.get("Adventurous") {
                    pl_name_wild.set(n.clone());
                }
            }
            settings_loaded.set(true);
        }
    });

    let fetch_folders = move || async move {
        match auth.call(get_user_folders()).await {
            Ok(fetched_folders) => folders.set(fetched_folders),
            Err(e) => error.set(format!("Failed to fetch folders: {e}")),
        }
    };

    use_future(move || async move {
        fetch_folders().await;
    });

    let handle_add_folder = move |_| async move {
        error.set("".to_string());
        success_msg.set("".to_string());

        if folder_name().is_empty() || folder_path().is_empty() {
            error.set("Name and Path are required".to_string());
            return;
        }

        match auth
            .call(create_user_folder(folder_name(), folder_path()))
            .await
        {
            Ok(_) => {
                success_msg.set("Folder added successfully".to_string());
                folder_name.set("".to_string());
                folder_path.set("".to_string());
                fetch_folders().await;
            }
            Err(e) => error.set(format!("Failed to add folder: {e}")),
        }
    };

    let handle_delete_folder = move |id: String| async move {
        match auth.call(delete_folder(id)).await {
            Ok(_) => {
                success_msg.set("Folder deleted successfully".to_string());
                fetch_folders().await;
            }
            Err(e) => error.set(format!("Failed to delete folder: {e}")),
        }
    };

    let handle_update_folder = move |id: String| async move {
        match auth
            .call(update_folder(id, edit_folder_name(), edit_folder_path()))
            .await
        {
            Ok(_) => {
                success_msg.set("Folder updated successfully".to_string());
                editing_folder_id.set(None);
                fetch_folders().await;
            }
            Err(e) => error.set(format!("Failed to update folder: {e}")),
        }
    };

    rsx! {
        div { class: "space-y-6",
        div { class: "bg-beet-panel border border-white/10 p-6 rounded-lg shadow-2xl relative z-10",
            h2 { class: "text-xl font-bold mb-4 text-beet-accent font-display", "Manage Music Folders" }

            // Local Messages
            if !error().is_empty() {
                div { class: "mb-4 p-4 bg-red-900/20 border border-red-500/50 rounded text-red-400 font-mono text-sm",
                    "{error}"
                }
            }
            if !success_msg().is_empty() {
                div { class: "mb-4 p-4 bg-green-900/20 border border-green-500/50 rounded text-green-400 font-mono text-sm",
                    "{success_msg}"
                }
            }

            div { class: "grid grid-cols-1 md:grid-cols-2 gap-4 mb-4",
                div {
                    label { class: "block text-xs font-mono text-gray-400 mb-1 uppercase tracking-wider",
                        "Folder Name (e.g., 'Music/Common')"
                    }
                    input {
                        class: "w-full p-2 rounded bg-beet-dark border border-white/10 focus:border-beet-accent focus:outline-none text-white font-mono",
                        value: "{folder_name}",
                        oninput: move |e| folder_name.set(e.value()),
                        placeholder: "My Music",
                        "type": "text",
                    }
                }
                div {
                    label { class: "block text-xs font-mono text-gray-400 mb-1 uppercase tracking-wider",
                        "Folder Path"
                    }
                    input {
                        class: "w-full p-2 rounded bg-beet-dark border border-white/10 focus:border-beet-accent focus:outline-none text-white font-mono",
                        value: "{folder_path}",
                        oninput: move |e| folder_path.set(e.value()),
                        placeholder: "/home/user/Music",
                        "type": "text",
                    }
                }
            }

            button { class: "retro-btn mb-6 rounded", onclick: handle_add_folder, "Add Folder" }

            // Existing Folders List
            h3 { class: "text-lg font-bold mb-2 text-white font-display border-b border-white/10 pb-2",
                "Existing Folders"
            }
            if folders.read().is_empty() {
                p { class: "text-gray-500 font-mono italic", "No folders added yet." }
            } else {
                ul { class: "space-y-2",
                    {
                        folders
                            .read()
                            .clone()
                            .into_iter()
                            .map(|folder| {
                                let id_edit = folder.id.clone();
                                let id_delete = folder.id.clone();
                                let id_update = folder.id.clone();
                                rsx! {
                                    li { class: "bg-white/5 border border-white/5 p-3 rounded hover:border-beet-accent/30 transition-colors",
                                        if editing_folder_id() == Some(folder.id.clone()) {
                                            div { class: "flex flex-col gap-2",
                                                input {
                                                    class: "p-2 rounded bg-beet-dark border border-white/10 focus:border-beet-accent text-white font-mono text-sm",
                                                    value: "{edit_folder_name}",
                                                    oninput: move |e| edit_folder_name.set(e.value()),
                                                    placeholder: "Name",
                                                }
                                                input {
                                                    class: "p-2 rounded bg-beet-dark border border-white/10 focus:border-beet-accent text-white font-mono text-sm",
                                                    value: "{edit_folder_path}",
                                                    oninput: move |e| edit_folder_path.set(e.value()),
                                                    placeholder: "Path",
                                                }
                                                div { class: "flex gap-2 mt-2",
                                                    button {
                                                        class: "text-xs uppercase tracking-wider font-bold text-beet-leaf hover:text-white transition-colors",
                                                        onclick: move |_| handle_update_folder(id_update.clone()),
                                                        "[ Save ]"
                                                    }
                                                    button {
                                                        class: "text-xs uppercase tracking-wider font-bold text-gray-500 hover:text-white transition-colors",
                                                        onclick: move |_| editing_folder_id.set(None),
                                                        "[ Cancel ]"
                                                    }
                                                }
                                            }
                                        } else {
                                            div { class: "flex justify-between items-center",
                                                div {
                                                    span { class: "font-bold text-white block font-display", "{folder.name}" }
                                                    span { class: "text-gray-500 text-xs font-mono", "{folder.path}" }
                                                }
                                                div { class: "flex gap-3",
                                                    button {
                                                        class: "text-xs font-mono text-gray-400 hover:text-beet-accent transition-colors underline decoration-dotted",
                                                        onclick: move |_| {
                                                            edit_folder_name.set(folder.name.clone());
                                                            edit_folder_path.set(folder.path.clone());
                                                            editing_folder_id.set(Some(id_edit.clone()));
                                                        },
                                                        "Edit"
                                                    }
                                                    button {
                                                        class: "text-xs font-mono text-gray-400 hover:text-red-400 transition-colors underline decoration-dotted",
                                                        onclick: move |_| handle_delete_folder(id_delete.clone()),
                                                        "Delete"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            })
                    }
                }
            }
        }

        // Library Settings
        div { class: "bg-beet-panel border border-white/10 p-6 rounded-lg shadow-2xl relative z-10 mt-6",
            h2 { class: "text-xl font-bold mb-4 text-beet-accent font-display", "Library Settings" }

            if !settings_loaded() {
                div { class: "animate-pulse text-gray-400 font-mono", "Loading..." }
            } else {
                div { class: "space-y-4",
                    // Discovery (user-level)
                    div {
                        h3 { class: "text-sm font-semibold text-white mb-3", "Discovery" }
                        p { class: "text-xs text-gray-500 font-mono mb-4",
                            "Automatically download new tracks for listening based on your scrobble history."
                        }
                        div { class: "space-y-3",
                            // Enable/disable toggle
                            div { class: "flex items-center justify-between p-3 bg-beet-dark rounded border border-white/10",
                                div {
                                    p { class: "text-sm text-white font-medium", "Enable Discovery" }
                                    p { class: "text-xs text-gray-500 font-mono mt-0.5",
                                        "Periodically generate a playlist of new tracks to try"
                                    }
                                }
                                button {
                                    class: format!(
                                        "relative w-11 h-6 rounded-full cursor-pointer transition-colors {}",
                                        if discovery_enabled() { "bg-beet-leaf" } else { "bg-gray-600" }
                                    ),
                                    onclick: move |_| async move {
                                        let new_val = !discovery_enabled();
                                        discovery_enabled.set(new_val);
                                        let update = api::UpdateUserSettings {
                                            discovery_enabled: Some(new_val),
                                            ..Default::default()
                                        };
                                        let _ = api::update_user_settings(update).await;
                                    },
                                    span {
                                        class: format!(
                                            "absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full transition-transform {}",
                                            if discovery_enabled() { "translate-x-5" } else { "" }
                                        ),
                                    }
                                }
                            }

                            if discovery_enabled() {
                                // Folder selector
                                div { class: "p-3 bg-beet-dark rounded border border-white/10",
                                    label { class: "block text-xs font-mono text-gray-400 mb-1 uppercase tracking-wider", "Download Folder" }
                                    if folders.read().is_empty() {
                                        p { class: "text-gray-500 font-mono text-sm italic", "No folders configured. Add a folder above first." }
                                    } else {
                                        select {
                                            class: "w-full p-2 rounded bg-beet-panel border border-white/10 text-white font-mono text-sm",
                                            value: "{discovery_folder_id}",
                                            onchange: move |e| async move {
                                                let val = e.value();
                                                discovery_folder_id.set(val.clone());
                                                let update = api::UpdateUserSettings {
                                                    discovery_folder_id: Some(val),
                                                    ..Default::default()
                                                };
                                                let _ = api::update_user_settings(update).await;
                                            },
                                            option { value: "", "Select a folder..." }
                                            for f in folders.read().iter() {
                                                option {
                                                    value: "{f.id}",
                                                    selected: discovery_folder_id() == f.id,
                                                    "{f.name} ({f.path})"
                                                }
                                            }
                                        }
                                    }
                                }

                                // Track count and lifetime
                                div { class: "flex gap-3",
                                    div { class: "flex-1 p-3 bg-beet-dark rounded border border-white/10",
                                        label { class: "block text-xs font-mono text-gray-400 mb-1 uppercase tracking-wider", "Track Count" }
                                        input {
                                            class: "w-full p-2 rounded bg-beet-panel border border-white/10 text-white font-mono text-sm text-center",
                                            "type": "number",
                                            min: "5",
                                            max: "50",
                                            value: "{discovery_track_count}",
                                            oninput: move |e| discovery_track_count.set(e.value()),
                                            onchange: move |_| async move {
                                                let val: i32 = discovery_track_count().parse().unwrap_or(20);
                                                let update = api::UpdateUserSettings {
                                                    discovery_track_count: Some(val),
                                                    ..Default::default()
                                                };
                                                let _ = api::update_user_settings(update).await;
                                            },
                                        }
                                    }
                                    div { class: "flex-1 p-3 bg-beet-dark rounded border border-white/10",
                                        label { class: "block text-xs font-mono text-gray-400 mb-1 uppercase tracking-wider", "Lifetime (days)" }
                                        input {
                                            class: "w-full p-2 rounded bg-beet-panel border border-white/10 text-white font-mono text-sm text-center",
                                            "type": "number",
                                            min: "1",
                                            max: "90",
                                            value: "{discovery_lifetime_days}",
                                            oninput: move |e| discovery_lifetime_days.set(e.value()),
                                            onchange: move |_| async move {
                                                let val: i32 = discovery_lifetime_days().parse().unwrap_or(7);
                                                let update = api::UpdateUserSettings {
                                                    discovery_lifetime_days: Some(val),
                                                    ..Default::default()
                                                };
                                                let _ = api::update_user_settings(update).await;
                                            },
                                        }
                                    }
                                }

                                // Playlists: toggle + name in one row per profile
                                div { class: "p-3 bg-beet-dark rounded border border-white/10",
                                    label { class: "block text-xs font-mono text-gray-400 mb-2 uppercase tracking-wider", "Playlists" }
                                    div { class: "space-y-2",
                                        for (label, value, color, signal) in [
                                            ("Safe", "Conservative", "border-blue-500/40 bg-blue-600/10", &pl_name_safe),
                                            ("Mix", "Balanced", "border-green-500/40 bg-green-600/10", &pl_name_mix),
                                            ("Wild", "Adventurous", "border-purple-500/40 bg-purple-600/10", &pl_name_wild),
                                        ] {
                                            {
                                                let active = discovery_profiles().split(',').any(|p| p.trim() == value);
                                                rsx! {
                                                    div {
                                                        class: format!(
                                                            "flex items-center gap-2 p-2 rounded border transition-colors {}",
                                                            if active { color } else { "border-white/5 bg-white/[0.02] opacity-50" }
                                                        ),
                                                        // Toggle button
                                                        button {
                                                            class: format!(
                                                                "shrink-0 w-16 py-0.5 text-xs font-mono rounded cursor-pointer transition-colors text-center {}",
                                                                if active { "bg-white/15 text-white" } else { "bg-white/5 text-gray-500" }
                                                            ),
                                                            onclick: move |_| async move {
                                                                let current: Vec<String> = discovery_profiles().split(',')
                                                                    .map(|s| s.trim().to_string())
                                                                    .filter(|s| !s.is_empty())
                                                                    .collect();
                                                                let new_profiles = if current.iter().any(|p| p == value) {
                                                                    let filtered: Vec<_> = current.into_iter().filter(|p| p != value).collect();
                                                                    if filtered.is_empty() { value.to_string() } else { filtered.join(",") }
                                                                } else {
                                                                    let mut updated = current;
                                                                    updated.push(value.to_string());
                                                                    updated.join(",")
                                                                };
                                                                discovery_profiles.set(new_profiles.clone());
                                                                let update = api::UpdateUserSettings {
                                                                    discovery_profiles: Some(new_profiles),
                                                                    ..Default::default()
                                                                };
                                                                let _ = api::update_user_settings(update).await;
                                                            },
                                                            "{label}"
                                                        }
                                                        // Name input
                                                        input {
                                                            class: "flex-1 p-1 rounded bg-beet-panel/50 border border-white/5 focus:border-beet-accent focus:outline-none text-white font-mono text-xs",
                                                            value: "{signal}",
                                                            disabled: !active,
                                                            oninput: {
                                                                let signal = *signal;
                                                                move |e: Event<FormData>| signal.clone().set(e.value())
                                                            },
                                                            onchange: move |_| async move {
                                                                let names = serde_json::json!({
                                                                    "Conservative": pl_name_safe(),
                                                                    "Balanced": pl_name_mix(),
                                                                    "Adventurous": pl_name_wild(),
                                                                });
                                                                let update = api::UpdateUserSettings {
                                                                    discovery_playlist_name: Some(names.to_string()),
                                                                    ..Default::default()
                                                                };
                                                                let _ = api::update_user_settings(update).await;
                                                            },
                                                            placeholder: value,
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Last.fm
                    div { class: "p-3 bg-beet-dark rounded border border-white/10",
                        p { class: "text-sm text-white font-medium mb-2", "Last.fm" }
                        div { class: "space-y-3",
                            div {
                                label { class: "block text-xs font-mono text-gray-400 mb-1 uppercase tracking-wider", "Username" }
                                input {
                                    class: "w-full p-2 rounded bg-beet-panel border border-white/10 focus:border-beet-accent focus:outline-none text-white font-mono text-sm",
                                    value: "{lastfm_username}",
                                    oninput: move |e| lastfm_username.set(e.value()),
                                    onchange: move |_| async move {
                                        let update = api::UpdateUserSettings {
                                            lastfm_username: Some(lastfm_username()),
                                            ..Default::default()
                                        };
                                        let _ = api::update_user_settings(update).await;
                                    },
                                    placeholder: "Your Last.fm username",
                                }
                            }
                            div {
                                label { class: "block text-xs font-mono text-gray-400 mb-1 uppercase tracking-wider", "API Key" }
                                input {
                                    class: "w-full p-2 rounded bg-beet-panel border border-white/10 focus:border-beet-accent focus:outline-none text-white font-mono text-sm",
                                    value: "{lastfm_api_key}",
                                    oninput: move |e| lastfm_api_key.set(e.value()),
                                    onchange: move |_| async move {
                                        let update = api::UpdateUserSettings {
                                            lastfm_api_key: Some(lastfm_api_key()),
                                            ..Default::default()
                                        };
                                        let _ = api::update_user_settings(update).await;
                                    },
                                    placeholder: "Enter Last.fm API key",
                                    "type": "password",
                                }
                                p { class: "text-xs text-gray-500 font-mono mt-1",
                                    "Get one at "
                                    a {
                                        href: "https://www.last.fm/api/account/create",
                                        target: "_blank",
                                        class: "text-beet-accent hover:underline",
                                        "last.fm/api"
                                    }
                                }
                            }
                        }
                    }

                    // ListenBrainz credentials
                    div { class: "p-3 bg-beet-dark rounded border border-white/10",
                        div {
                            p { class: "text-sm text-white font-medium mb-1", "ListenBrainz Username" }
                            input {
                                class: "w-full p-2 rounded bg-beet-panel border border-white/10 focus:border-beet-accent focus:outline-none text-white font-mono text-sm",
                                value: "{lb_username}",
                                oninput: move |e| lb_username.set(e.value()),
                                onchange: move |_| async move {
                                    let update = api::UpdateUserSettings {
                                        listenbrainz_username: Some(lb_username()),
                                        ..Default::default()
                                    };
                                    let _ = api::update_user_settings(update).await;
                                },
                                placeholder: "Enter ListenBrainz username",
                                "type": "text",
                            }
                        }
                        div { class: "mt-3",
                            p { class: "text-sm text-white font-medium mb-1", "ListenBrainz Token" }
                            p { class: "text-xs text-gray-500 font-mono mb-2",
                                "Configure scrobbling in your Navidrome personal settings."
                            }
                            input {
                                class: "w-full p-2 rounded bg-beet-panel border border-white/10 focus:border-beet-accent focus:outline-none text-white font-mono text-sm",
                                value: "{lb_token}",
                                oninput: move |e| lb_token.set(e.value()),
                                onchange: move |_| async move {
                                    let update = api::UpdateUserSettings {
                                        listenbrainz_token: Some(lb_token()),
                                        ..Default::default()
                                    };
                                    let _ = api::update_user_settings(update).await;
                                },
                                placeholder: "Enter ListenBrainz token",
                                "type": "password",
                            }
                        }
                        p { class: "text-xs text-gray-500 font-mono mt-3",
                            "Set up scrobbling in your Navidrome personal settings to feed the recommendation engine."
                        }
                    }

                    // Auto-delete toggle
                    div { class: "flex items-center justify-between p-3 bg-beet-dark rounded border border-white/10",
                        div {
                            p { class: "text-sm text-white font-medium", "Auto-delete 1-star tracks" }
                            p { class: "text-xs text-gray-500 font-mono mt-0.5",
                                "Automatically delete files rated 1 star in Navidrome during sync"
                            }
                        }
                        button {
                            class: format!(
                                "relative w-11 h-6 rounded-full cursor-pointer transition-colors {}",
                                if auto_delete_enabled() { "bg-red-500" } else { "bg-gray-600" }
                            ),
                            onclick: move |_| async move {
                                let new_val = !auto_delete_enabled();
                                auto_delete_enabled.set(new_val);
                                let update = api::UpdateUserSettings {
                                    auto_delete_enabled: Some(new_val),
                                    ..Default::default()
                                };
                                let _ = api::update_user_settings(update).await;
                            },
                            span {
                                class: format!(
                                    "absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full transition-transform {}",
                                    if auto_delete_enabled() { "translate-x-5" } else { "" }
                                ),
                            }
                        }
                    }

                    // Discovery promote threshold
                    div { class: "flex items-center justify-between p-3 bg-beet-dark rounded border border-white/10",
                        div {
                            p { class: "text-sm text-white font-medium", "Discovery promote threshold" }
                            p { class: "text-xs text-gray-500 font-mono mt-0.5",
                                "Promote discovery tracks to your library when rated at or above this"
                            }
                        }
                        input {
                            class: "w-16 p-2 rounded bg-beet-panel border border-white/10 text-white font-mono text-sm text-center",
                            value: "{discovery_promote_threshold}",
                            oninput: move |e| discovery_promote_threshold.set(e.value()),
                            onchange: move |_| async move {
                                let val: u8 = discovery_promote_threshold().parse().unwrap_or(3);
                                let update = api::UpdateUserSettings {
                                    discovery_promote_threshold: Some(val),
                                    ..Default::default()
                                };
                                let _ = api::update_user_settings(update).await;
                            },
                            "type": "number",
                            min: "1",
                            max: "5",
                        }
                    }
                }
            }
        }
        }
    }
}
