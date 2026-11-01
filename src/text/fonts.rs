use std::{collections::BTreeSet, sync::LazyLock};

use fontdb::{Family, Query, Stretch, Style, Weight};

use crate::{Error, Result, text::Font, text::FontOptions};

#[cfg(target_os = "windows")]
const SYSTEM_FONTS_DIRECTORY: &str = "C:/Windows/Fonts";
#[cfg(target_os = "linux")]
const SYSTEM_FONTS_DIRECTORY: &str = "/usr/share/fonts";
#[cfg(target_os = "android")]
const SYSTEM_FONTS_DIRECTORY: &str = "/system/fonts";
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos"
))]
const SYSTEM_FONTS_DIRECTORY: &str = "/System/Library/Fonts";

static SYSTEM_FONTS: LazyLock<fontdb::Database> = LazyLock::new(|| {
    let mut database = fontdb::Database::new();
    database.load_fonts_dir(SYSTEM_FONTS_DIRECTORY);
    database
});

/// Gets the font family names available on the system.
pub fn system_font_names() -> Vec<String> {
    SYSTEM_FONTS
        .faces()
        .flat_map(|face| face.families.iter().map(|(name, _)| name.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Loads the Baskerville font family.
pub fn baskerville(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Baskerville")], options)
}

/// Loads the Arial font family.
pub fn arial(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Arial")], options)
}

/// Loads the Arial Black font family.
pub fn arial_black(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Arial Black")], options)
}

/// Loads the Comic Sans MS font family.
pub fn comic_sans_ms(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Comic Sans MS")], options)
}

/// Loads the Consolas font family.
pub fn consolas(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Consolas")], options)
}

/// Loads the Courier New font family.
pub fn courier_new(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Courier New")], options)
}

/// Loads the cursive font family.
pub fn cursive(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Cursive], options)
}

/// Loads the fantasy font family.
pub fn fantasy(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Fantasy], options)
}

/// Loads the Garamond font family.
pub fn garamond(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Garamond")], options)
}

/// Loads the Georgia font family.
pub fn georgia(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Georgia")], options)
}

/// Loads the Impact font family.
pub fn impact(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Impact")], options)
}

/// Loads the Inter font family.
pub fn inter(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Inter")], options)
}

/// Loads the Lato font family.
pub fn lato(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Lato")], options)
}

/// Loads the Menlo font family.
pub fn menlo(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Menlo")], options)
}

/// Loads the Monaco font family.
pub fn monaco(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Monaco")], options)
}

/// Loads the monospace font family.
pub fn monospace(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Monospace], options)
}

/// Loads the Noto Sans font family.
pub fn noto_sans(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Noto Sans")], options)
}

/// Loads the Noto Sans Mono font family.
pub fn noto_sans_mono(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Noto Sans Mono")], options)
}

/// Loads the Noto Serif font family.
pub fn noto_serif(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Noto Serif")], options)
}

/// Loads the Open Sans font family.
pub fn open_sans(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Open Sans")], options)
}

/// Loads the Palatino font family.
pub fn palatino(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Palatino")], options)
}

/// Loads the Roboto font family.
pub fn roboto(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Roboto")], options)
}

/// Loads the SF Pro font family.
pub fn sf_pro(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("SF Pro")], options)
}

/// Loads the Segoe UI font family.
pub fn segoe_ui(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Segoe UI")], options)
}

/// Loads the serif font family.
pub fn serif(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Serif], options)
}

/// Loads the Source Code Pro font family.
pub fn source_code_pro(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Source Code Pro")], options)
}

/// Loads the sans-serif font family.
pub fn sans_serif(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::SansSerif], options)
}

/// Loads the Ubuntu font family.
pub fn ubuntu(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Ubuntu")], options)
}

/// Loads the DejaVu Sans font family.
pub fn dejavu_sans(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("DejaVu Sans")], options)
}

/// Loads the Helvetica Neue font family.
pub fn helvetica_neue(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Helvetica Neue")], options)
}

/// Loads the Tahoma font family.
pub fn tahoma(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Tahoma")], options)
}

/// Loads the Times New Roman font family.
pub fn times_new_roman(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Times New Roman")], options)
}

/// Loads the Trebuchet MS font family.
pub fn trebuchet_ms(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Trebuchet MS")], options)
}

/// Loads the Verdana font family.
pub fn verdana(options: &FontOptions) -> Result<Font> {
    load_font(&[Family::Name("Verdana")], options)
}

fn load_font(families: &[Family<'_>], options: &FontOptions) -> Result<Font> {
    let query = Query {
        families,
        weight: Weight::NORMAL,
        stretch: Stretch::Normal,
        style: Style::Normal,
    };
    let id = SYSTEM_FONTS
        .query(&query)
        .ok_or_else(|| Error::font("Font not found."))?;
    let data = SYSTEM_FONTS
        .with_face_data(id, |font_data, _| font_data.to_vec())
        .ok_or_else(|| Error::font("Failed to read the font."))?;

    Font::new(data, options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fonts() {
        fn assert_family(family_name: &'static str, result: Result<Font>) {
            let query = Query {
                families: &[Family::Name(family_name)],
                weight: Weight::NORMAL,
                stretch: Stretch::Normal,
                style: Style::Normal,
            };
            let is_installed = SYSTEM_FONTS.query(&query).is_some();

            match result {
                Ok(_) => assert!(is_installed, "{family_name} should not fall back"),
                Err(Error::Font(message)) if message == "Font not found." => {
                    assert!(
                        !is_installed,
                        "{family_name} is installed but failed to load"
                    )
                }
                Err(error) => panic!("Failed to load {family_name}: {error}"),
            }
        }

        assert_family("Arial", arial(&FontOptions::default()));
        assert_family("Arial Black", arial_black(&FontOptions::default()));
        assert_family("Baskerville", baskerville(&FontOptions::default()));
        assert_family("Comic Sans MS", comic_sans_ms(&FontOptions::default()));
        assert_family("Consolas", consolas(&FontOptions::default()));
        assert_family("Courier New", courier_new(&FontOptions::default()));
        assert_family("DejaVu Sans", dejavu_sans(&FontOptions::default()));
        assert_family("Garamond", garamond(&FontOptions::default()));
        assert_family("Georgia", georgia(&FontOptions::default()));
        assert_family("Helvetica Neue", helvetica_neue(&FontOptions::default()));
        assert_family("Impact", impact(&FontOptions::default()));
        assert_family("Inter", inter(&FontOptions::default()));
        assert_family("Lato", lato(&FontOptions::default()));
        assert_family("Menlo", menlo(&FontOptions::default()));
        assert_family("Monaco", monaco(&FontOptions::default()));
        assert_family("Noto Sans", noto_sans(&FontOptions::default()));
        assert_family("Noto Sans Mono", noto_sans_mono(&FontOptions::default()));
        assert_family("Noto Serif", noto_serif(&FontOptions::default()));
        assert_family("Open Sans", open_sans(&FontOptions::default()));
        assert_family("Palatino", palatino(&FontOptions::default()));
        assert_family("Roboto", roboto(&FontOptions::default()));
        assert_family("SF Pro", sf_pro(&FontOptions::default()));
        assert_family("Segoe UI", segoe_ui(&FontOptions::default()));
        assert_family("Source Code Pro", source_code_pro(&FontOptions::default()));
        assert_family("Tahoma", tahoma(&FontOptions::default()));
        assert_family("Times New Roman", times_new_roman(&FontOptions::default()));
        assert_family("Trebuchet MS", trebuchet_ms(&FontOptions::default()));
        assert_family("Ubuntu", ubuntu(&FontOptions::default()));
        assert_family("Verdana", verdana(&FontOptions::default()));

        assert!(cursive(&FontOptions::default()).is_ok());
        assert!(fantasy(&FontOptions::default()).is_ok());
        assert!(monospace(&FontOptions::default()).is_ok());
        assert!(serif(&FontOptions::default()).is_ok());
        assert!(sans_serif(&FontOptions::default()).is_ok());
    }
}
