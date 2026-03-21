use dioxus::prelude::*;
use shared::system::SystemHealth;

#[component]
pub fn SystemStatus(health: SystemHealth) -> Element {
    rsx! {
      div { class: "flex justify-center gap-6 text-xs font-mono text-gray-500 flex-wrap",
        span { class: "flex items-center gap-2",
          span {
            class: format!(
                "w-2 h-2 rounded-full {}",
                if health.downloader_online { "bg-beet-leaf animate-pulse" } else { "bg-red-500" },
            ),
          }
          if health.downloader_online {
            "DOWNLOADER ONLINE"
          } else {
            "DOWNLOADER OFFLINE"
          }
        }
        span { class: "flex items-center gap-2",
          span {
            class: format!(
                "w-2 h-2 rounded-full {}",
                if health.beets_ready { "bg-beet-accent" } else { "bg-red-500" },
            ),
          }
          if health.beets_ready {
            "BEETS READY"
          } else {
            "BEETS MISSING"
          }
        }
        span { class: "flex items-center gap-2",
          span {
            class: format!(
                "w-2 h-2 rounded-full {}",
                if health.navidrome_online { "bg-blue-400 animate-pulse" } else { "bg-red-500" },
            ),
          }
          if health.navidrome_online {
            "NAVIDROME ONLINE"
          } else {
            "NAVIDROME OFFLINE"
          }
        }
      }
    }
}
