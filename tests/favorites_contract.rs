use crabpdf::{
    application::favorites::{self, FavoriteAction, FavoriteEffect},
    config::{Favorite, Preferences},
    document::{DocumentInfo, PageColors, Zoom},
    layout::LayoutMode,
    state::AppState,
};

#[test]
fn legacy_profile_records_use_the_extracted_type_and_keep_the_view_on_navigation() {
    let profile = r#"
[[favorites]]
path = "Lecturas/niño.epub"
page = 7
label = "Introducción"
[[favorites]]
path = "other.pdf"
page = 1
label = "Other"
"#;
    let mut preferences: Preferences = toml::from_str(profile).unwrap();
    // The facade exposes the same type; there is no conversion or second store.
    let _: &Favorite = &preferences.favorites[0];
    let mut state = AppState::default();
    state.begin_open_with_id(17);
    state.finish_open(
        17,
        Ok(DocumentInfo {
            path: "Lecturas/niño.epub".into(),
            page_count: 10,
        }),
    );
    state.zoom = Zoom::Percent(125);
    state.rotation = 3;
    state.layout = LayoutMode::Book;
    state.continuous = true;
    state.manga = true;
    state.colors = PageColors::Grayscale;
    let document = &state.document.as_ref().unwrap().info;
    let FavoriteEffect::Navigate(page) = favorites::apply(
        &mut preferences.favorites,
        document,
        FavoriteAction::Navigate(7),
    ) else {
        panic!("Saved favorite must produce a navigation destination");
    };
    state.go_to(page, true);
    assert_eq!(state.document.as_ref().unwrap().page, 7);
    assert_eq!(state.zoom, Zoom::Percent(125));
    assert_eq!(state.rotation, 3);
    assert_eq!(state.layout, LayoutMode::Book);
    assert!(state.continuous && state.manga);
    assert_eq!(state.colors, PageColors::Grayscale);
    let restored: Preferences = toml::from_str(&toml::to_string(&preferences).unwrap()).unwrap();
    assert_eq!(restored.favorites, preferences.favorites);
    assert_eq!(restored.favorites[0].label, "Introducción");
    assert_eq!(restored.favorites[1].path.to_str(), Some("other.pdf"));
}
