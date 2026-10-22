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
            preset: Preset::WindowsClassic,
        },
        Fonts::Preset {
            preset: Preset::MacosBbs,
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
        Fonts::Preset {
            preset: Preset::Bundled,
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
    assert_eq!(macos["bundled"].ascii_font, "Cascadia Mono");
    assert_eq!(macos["bundled"].cjk_font, "Noto Sans CJK TC");
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

#[test]
fn traditional_bbs_presets_keep_latin_and_cjk_together_without_changing_existing_choices() {
    // Independent portable input, including one saved on a different OS.
    let bytes=br#"{"format":"PTTStella.DisplayProfile","version":1,"name":"Traditional BBS","display":{"fonts":{"kind":"preset","preset":"windows_classic"},"font_size":22,"baseline":20}}"#;
    let portable = PortableProfile::decode(bytes).unwrap();
    let display = portable.display.resolve().unwrap();
    assert_eq!(
        (&*display.ascii_font, &*display.cjk_font),
        ("MingLiU", "MingLiU")
    );
    assert_eq!(display.font_size, 22.0);
    let app = AppSettings {
        display: portable.display,
        ..Default::default()
    };
    assert_eq!(
        AppSettings::decode(&serde_json::to_vec(&app).unwrap()).unwrap(),
        app
    );
    let presets = DisplaySettings::presets();
    assert_eq!(presets["macos_bbs"].ascii_font, "Songti TC");
    assert_eq!(presets["macos_bbs"].cjk_font, "Songti TC");
    assert_eq!(presets["macos_serif"].ascii_font, "Menlo");
    assert_eq!(presets["windows"].ascii_font, "Cascadia Mono");
    assert_eq!(presets["windows"].cjk_font, "Microsoft JhengHei");
}

#[test]
fn opacity_migrates_without_changing_other_preferences_and_rejects_invalid_values() {
    use pttstella::settings::ToolOpacity;
    for version in [2, 3] {
        let bytes = format!(
            r#"{{"version":{version},"display":{{"fonts":{{"kind":"preset","preset":"bundled"}},"font_size":18,"baseline":19}},"hover":"off","language":"en-US"}}"#
        );
        let settings = AppSettings::decode(bytes.as_bytes()).unwrap();
        assert_eq!(settings.version, 6);
        assert_eq!(settings.tools_opacity.percent(), 100);
        assert_eq!(settings.display.font_size, 18.0);
        assert_eq!(settings.language, pttstella::i18n::Language::English);
    }
    for percent in [20, 65, 100] {
        let settings = AppSettings {
            tools_opacity: ToolOpacity::try_from(percent).unwrap(),
            ..Default::default()
        };
        assert_eq!(
            AppSettings::decode(&serde_json::to_vec(&settings).unwrap())
                .unwrap()
                .tools_opacity
                .percent(),
            percent
        );
        let profile = PortableProfile::new("no tools".into(), settings.display)
            .unwrap()
            .encode()
            .unwrap();
        assert!(!String::from_utf8(profile).unwrap().contains("opacity"));
    }
    let settings = serde_json::to_value(AppSettings::default()).unwrap();
    for bad in [
        serde_json::json!(0),
        serde_json::json!(19),
        serde_json::json!(101),
        serde_json::json!(40.5),
        serde_json::json!("40"),
    ] {
        let mut value = settings.clone();
        value["tools_opacity"] = bad;
        assert!(AppSettings::decode(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}

#[test]
fn tool_access_migrates_defaults_and_persists_independent_choices_strictly() {
    use pttstella::settings::ToolAccess;
    for version in [2, 3, 4] {
        let old = serde_json::json!({"version":version,"display":{"fonts":{"kind":"platform_default"},"font_size":18,"baseline":19},"hover":"off","tools_opacity":65});
        let app = AppSettings::decode(&serde_json::to_vec(&old).unwrap()).unwrap();
        assert_eq!(app.version, 6);
        assert_eq!(app.tools_access, ToolAccess::default());
        assert_eq!(app.tools_opacity.percent(), 65);
    }
    for right_click_menu in [false, true] {
        for show_floating_button in [false, true] {
            let app = AppSettings {
                tools_access: ToolAccess {
                    right_click_menu,
                    show_floating_button,
                },
                ..Default::default()
            };
            assert_eq!(
                AppSettings::decode(&serde_json::to_vec(&app).unwrap()).unwrap(),
                app
            );
            let profile = PortableProfile::new("display only".into(), app.display)
                .unwrap()
                .encode()
                .unwrap();
            assert!(!String::from_utf8(profile).unwrap().contains("tools_access"));
        }
    }
    for access in [
        serde_json::json!({"right_click_menu":"true","show_floating_button":true}),
        serde_json::json!({"right_click_menu":true}),
        serde_json::json!({"right_click_menu":true,"show_floating_button":true,"extra":false}),
    ] {
        let mut app = serde_json::to_value(AppSettings::default()).unwrap();
        app["tools_access"] = access;
        assert!(AppSettings::decode(&serde_json::to_vec(&app).unwrap()).is_err());
    }
}

#[test]
fn article_mode_migrates_and_round_trips_without_accepting_unknown_modes() {
    use pttstella::settings::{AppSettings, ArticleMode};
    for version in 2..=5 {
        let value = serde_json::json!({"version":version,"display":{"fonts":{"kind":"platform_default"},"font_size":18,"baseline":19},"hover":"off"});
        let loaded = AppSettings::decode(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(loaded.version, 6);
        assert_eq!(loaded.article_mode, ArticleMode::Web);
    }
    let terminal = AppSettings {
        article_mode: ArticleMode::Terminal,
        ..Default::default()
    };
    let mut value = serde_json::to_value(&terminal).unwrap();
    assert_eq!(
        AppSettings::decode(&serde_json::to_vec(&value).unwrap()).unwrap(),
        terminal
    );
    value["article_mode"] = "unknown".into();
    assert!(AppSettings::decode(&serde_json::to_vec(&value).unwrap()).is_err());
}

#[test]
fn article_boundary_preference_defaults_off_migrates_and_round_trips() {
    use pttstella::settings::AppSettings;
    for version in 2..=6 {
        let mut value = serde_json::to_value(AppSettings::default()).unwrap();
        value["version"] = version.into();
        value
            .as_object_mut()
            .unwrap()
            .remove("article_boundary_navigation");
        let old = AppSettings::decode(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(!old.article_boundary_navigation);
    }
    let enabled = AppSettings {
        article_boundary_navigation: true,
        ..Default::default()
    };
    assert_eq!(
        AppSettings::decode(&serde_json::to_vec(&enabled).unwrap()).unwrap(),
        enabled
    );
}
