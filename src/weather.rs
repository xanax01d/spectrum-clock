use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Polls current weather in a background thread via `wttr.in` without blocking the UI.
pub struct WeatherWatcher {
    current: Arc<Mutex<Option<String>>>,
}

impl WeatherWatcher {
    pub fn start(city: Option<String>) -> Self {
        let current = Arc::new(Mutex::new(None));
        let shared = Arc::clone(&current);

        std::thread::Builder::new()
            .name("weather-watcher".to_string())
            .spawn(move || {
                let agent = ureq::AgentBuilder::new()
                    .timeout(Duration::from_secs(4))
                    .user_agent("curl/8.0")
                    .build();

                let url = match city.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                    Some(c) => format!("https://wttr.in/{c}?format=%t+%C"),
                    None => "https://wttr.in/?format=%t+%C".to_string(),
                };

                loop {
                    if let Some(summary) = fetch_weather(&agent, &url)
                        && let Ok(mut guard) = shared.lock()
                    {
                        *guard = Some(summary);
                    }
                    std::thread::sleep(Duration::from_secs(30 * 60));
                }
            })
            .ok();

        Self { current }
    }

    pub fn latest(&self) -> Option<String> {
        self.current.try_lock().ok().and_then(|g| g.clone())
    }
}

fn fetch_weather(agent: &ureq::Agent, url: &str) -> Option<String> {
    let resp = agent.get(url).call().ok()?;
    let text = resp.into_string().ok()?;
    let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() || cleaned.contains("Unknown") || cleaned.len() > 48 {
        None
    } else {
        Some(cleaned)
    }
}
