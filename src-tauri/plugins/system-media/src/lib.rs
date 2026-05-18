use serde::{Deserialize, Serialize};
use tauri::{plugin::TauriPlugin, Manager, Runtime};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "app.tauri.bayin.systemmedia";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMediaConfig {
    pub enabled: bool,
}

pub struct SystemMedia<R: Runtime> {
    _marker: std::marker::PhantomData<fn() -> R>,
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R, Option<SystemMediaConfig>> {
    tauri::plugin::Builder::<R, Option<SystemMediaConfig>>::new("system-media")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            {
                // We don't need a mobile `PluginHandle` yet because all transport controls are handled
                // via JNI from the ForegroundService.
                let _ = api.register_android_plugin(PLUGIN_IDENTIFIER, "SystemMediaPlugin")?;
            }

            // Desktop + iOS placeholder for future system media integrations.
            app.manage(SystemMedia {
                _marker: std::marker::PhantomData::<fn() -> R>,
            });

            let _ = api;
            Ok(())
        })
        .build()
}
