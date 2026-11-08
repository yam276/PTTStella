//! Read the existing profile and station list without mutating either store.
use pttstella::sites::session::State;
use pttstella::sites::{Sites, Target};
use slint::SharedString;
use std::{cell::RefCell, rc::Rc};

pub struct Stations {
    pub entries: Vec<(SharedString, Target)>,
    pub warnings: Vec<String>,
    pub state: Rc<RefCell<State>>,
}

impl Stations {
    pub fn load(profile: &crate::profile::Profile) -> Self {
        let mut warnings = Vec::new();
        let state = State::load(Ok(profile.sites()), false);
        if let Some(error) = state.warning() {
            warnings.push(error.to_owned());
        }
        let sites = state.sites().clone();
        let (entries, warning) = Self::choices(sites);
        warnings.extend(warning);
        Self {
            entries,
            warnings,
            state: Rc::new(RefCell::new(state)),
        }
    }

    pub fn choices(sites: Sites) -> (Vec<(SharedString, Target)>, Option<String>) {
        let warning = sites.default.validate_connection(false).err();
        let default = if warning.is_some() {
            Target::default()
        } else {
            sites.default
        };
        let label = default.endpoint.as_str().into();
        let mut entries = vec![(label, default.clone())];
        entries.extend(sites.favourites.into_iter().filter_map(|favourite| {
            (favourite.target != default && favourite.target.validate_connection(false).is_ok())
                .then(|| (favourite.name.into(), favourite.target))
        }));
        (entries, warning)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pttstella::{encoding::TextEncoding, sites::Favourite};

    #[test]
    fn default_comes_first_and_legacy_insecure_choices_are_not_dialled() {
        let legacy = Target::new("ws://example.org/bbs".into(), TextEncoding::Uao).unwrap();
        let valid = Target::new("wss://example.org/bbs".into(), TextEncoding::Gbk).unwrap();
        let mut sites = Sites {
            default: valid.clone(),
            ..Sites::default()
        };
        sites.favourites.push(Favourite {
            name: "Legacy".into(),
            target: legacy.clone(),
        });
        sites.favourites.push(Favourite {
            name: "Valid".into(),
            target: valid.clone(),
        });
        let (entries, warning) = Stations::choices(sites.clone());
        assert!(warning.is_none());
        assert_eq!(entries[0].1, valid);
        assert_eq!(
            entries
                .iter()
                .filter(|(_, target)| *target == valid)
                .count(),
            1
        );
        assert!(!entries.iter().any(|(_, target)| *target == legacy));
        sites.default = legacy;
        let (entries, warning) = Stations::choices(sites);
        assert!(warning.is_some());
        assert_eq!(entries[0].1, Target::default());
    }
}
