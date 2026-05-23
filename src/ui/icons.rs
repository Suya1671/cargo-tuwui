use std::{fmt::Display, sync::LazyLock};

static HAS_NERD_ICONS: LazyLock<bool> = LazyLock::new(|| {
    let env_vars: Vec<(String, String)> = std::env::vars().collect();
    has_nerd_font::detect(&env_vars).detected.unwrap_or(false)
});

pub enum Icons {
    Loading,
    Done,
    Error,
    Update,
    Warning,
}

impl Icons {
    pub const fn get_nerd_icon(&self) -> &'static str {
        match self {
            Self::Loading => "󰑓",
            Self::Done => "",
            Self::Error => "",
            Self::Update => "",
            Self::Warning => "",
        }
    }

    pub const fn get_emoji_icon(&self) -> &'static str {
        match self {
            Self::Loading => "⏳",
            Self::Done => "✔️",
            Self::Error => "⛔",
            Self::Update => "⬆️",
            Self::Warning => "⚠️",
        }
    }

    pub fn get_icon(&self) -> &'static str {
        if *HAS_NERD_ICONS {
            self.get_nerd_icon()
        } else {
            self.get_emoji_icon()
        }
    }
}

impl Display for Icons {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.get_icon())
    }
}
