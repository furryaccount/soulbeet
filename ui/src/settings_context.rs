use api::{ProviderInfo, UpdateUserSettings, UserSettings};
use dioxus::prelude::*;

#[derive(PartialEq, Clone, Copy, Debug, Default)]
pub enum SearchType {
    #[default]
    Album,
    Track,
}

impl SearchType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SearchType::Album => "album",
            SearchType::Track => "track",
        }
    }
}

impl From<&str> for SearchType {
    fn from(s: &str) -> Self {
        match s {
            "track" => SearchType::Track,
            _ => SearchType::Album,
        }
    }
}

/// Settings context that provides access to user settings throughout the app.
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    state: Signal<Option<UserSettings>>,
    providers: Signal<Vec<ProviderInfo>>,
    loaded: Signal<bool>,
}

impl Settings {
    pub fn new(
        state: Signal<Option<UserSettings>>,
        providers: Signal<Vec<ProviderInfo>>,
        loaded: Signal<bool>,
    ) -> Self {
        Self {
            state,
            providers,
            loaded,
        }
    }

    /// Get the current user settings, if loaded.
    pub fn get(&self) -> Option<UserSettings> {
        self.state.read().clone()
    }

    /// Check if settings have finished loading.
    pub fn is_loaded(&self) -> bool {
        *self.loaded.read()
    }

    /// Check if settings are still loading (inverse of is_loaded for convenience).
    pub fn is_loading(&self) -> bool {
        !self.is_loaded()
    }

    /// Get the default metadata provider ID.
    pub fn default_provider(&self) -> String {
        self.state
            .read()
            .as_ref()
            .and_then(|s| s.default_metadata_provider.clone())
            .unwrap_or_else(|| "musicbrainz".to_string())
    }

    /// Get the last used search type.
    pub fn last_search_type(&self) -> SearchType {
        self.state
            .read()
            .as_ref()
            .and_then(|s| s.last_search_type.as_deref())
            .map(SearchType::from)
            .unwrap_or_default()
    }

    /// Get the list of available metadata providers.
    pub fn providers(&self) -> Vec<ProviderInfo> {
        self.providers.read().clone()
    }

    /// Update settings (call after successful API update).
    pub fn set(&mut self, settings: UserSettings) {
        self.state.set(Some(settings));
    }

    /// Update settings on the server and refresh local state.
    pub async fn update(
        &mut self,
        update: UpdateUserSettings,
    ) -> Result<UserSettings, ServerFnError> {
        let result = api::update_user_settings(update).await?;
        self.state.set(Some(result.clone()));
        Ok(result)
    }

    /// Convenience method to update just the last search type.
    pub async fn set_last_search_type(
        &mut self,
        search_type: SearchType,
    ) -> Result<(), ServerFnError> {
        let update = UpdateUserSettings {
            last_search_type: Some(search_type.as_str().to_string()),
            ..Default::default()
        };
        self.update(update).await?;
        Ok(())
    }

    /// Refresh the providers list from the server.
    pub async fn refresh_providers(&mut self) -> Result<(), ServerFnError> {
        let providers = api::get_metadata_providers().await?;
        self.providers.set(providers);
        Ok(())
    }
}

/// Hook to access the settings context.
pub fn use_settings() -> Settings {
    use_context::<Settings>()
}

/// Provider component that loads settings and makes them available via context.
#[component]
pub fn SettingsProvider(children: Element) -> Element {
    let settings_future = use_server_future(|| async move {
        let (settings_result, providers_result) =
            futures::join!(api::get_user_settings(), api::get_metadata_providers(),);
        (settings_result.ok(), providers_result.unwrap_or_default())
    })?;

    let (settings, providers) = settings_future.read().clone().unwrap_or_default();
    let settings_state = use_signal(|| settings);
    let providers_state = use_signal(|| providers);
    let loaded = use_signal(|| true);

    use_context_provider(|| Settings::new(settings_state, providers_state, loaded));

    rsx! { {children} }
}
