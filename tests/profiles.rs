use pttstella::settings::{
    AppSettings, DisplayProfile, DisplaySettings, Fonts, MAX_SETTINGS_BYTES, PortableProfile,
    Preset,
};
#[test]
fn fixed_presets_and_custom_profiles_roundtrip_without_platform_inference() {
    for fonts in [
        Fonts::Preset {
            preset: Preset::Windows,
        },
        Fonts::Preset {
            preset: Preset::Macos,
        },
        Fonts::Preset {
            preset: Preset::Linux,
        },
        Fonts::Preset {
            preset: Preset::MacosSerif,
        },
        Fonts::Custom {
            ascii_font: "My Mono".into(),
            cjk_font: "我的字型".into(),
        },
    ] {
        let display = DisplayProfile {
            fonts,
            font_size: 18.0,
            baseline: 19.0,
        };
        let profile = PortableProfile::new("合成設定".into(), display.clone()).unwrap();
        let bytes = profile.clone().encode().unwrap();
        assert_eq!(PortableProfile::decode(&bytes).unwrap(), profile);
        assert_eq!(display.resolve().unwrap().font_size, 18.0);
    }
    let windows = DisplayProfile {
        fonts: Fonts::Preset {
            preset: Preset::Windows,
        },
        ..Default::default()
    };
    assert_eq!(windows.resolve().unwrap().ascii_font, "Cascadia Mono");
    assert_eq!(windows.resolve().unwrap().cjk_font, "Microsoft JhengHei");
    let macos = DisplaySettings::presets();
    assert_eq!(macos["macos"].ascii_font, macos["macos_serif"].ascii_font);
    assert_eq!(macos["macos"].cjk_font, "PingFang TC");
    assert_eq!(macos["macos_serif"].cjk_font, "Songti TC");
}
#[test]
fn v1_explicit_names_always_migrate_to_custom_even_when_equal_to_defaults() {
    let old = DisplaySettings::default();
    let bytes = serde_json::to_vec(&old).unwrap();
    let migrated = AppSettings::decode(&bytes).unwrap();
    assert!(matches!(migrated.display.fonts, Fonts::Custom { .. }));
    assert_eq!(migrated.display.resolve().unwrap(), old);
    let new = serde_json::to_vec(&migrated).unwrap();
    assert_eq!(AppSettings::decode(&new).unwrap(), migrated);
}
#[test]
fn portable_format_rejects_duplicate_unknown_and_invalid_fields_as_a_whole() {
    let good = String::from_utf8(
        PortableProfile::new("A".into(), DisplayProfile::default())
            .unwrap()
            .encode()
            .unwrap(),
    )
    .unwrap();
    for bad in [
        good.replacen("\"version\": 1", "\"version\": 99", 1),
        good.replacen("\"name\": \"A\"", "\"name\": \"A\",\"name\":\"B\"", 1),
        good.replacen(
            "\"platform_default\"",
            "\"platform_default\",\"kind\":\"platform_default\"",
            1,
        ),
        good.replacen(
            "\"font_size\": 20.0",
            "\"font_size\": 20.0,\"font_size\":18",
            1,
        ),
        good.replacen("\"font_size\": 20.0", "\"font_size\": 100", 1),
        good.replacen(
            "\"platform_default\"",
            "\"platform_default\",\"extra\":1",
            1,
        ),
        good.replacen("\"name\": \"A\"", "\"name\": \"\"", 1),
    ] {
        assert!(PortableProfile::decode(bad.as_bytes()).is_err(), "{bad}");
    }
    assert!(PortableProfile::decode(&vec![b' '; MAX_SETTINGS_BYTES + 1]).is_err());
    assert!(
        PortableProfile::new(
            "A".into(),
            DisplayProfile {
                fonts: Fonts::Custom {
                    ascii_font: "bad\nname".into(),
                    cjk_font: "中".into()
                },
                ..Default::default()
            }
        )
        .is_err()
    );
}
