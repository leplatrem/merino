//! A suggestion provider that provides toy responses.
//!
//! It is useful in that it is fully self contained and very simple. It is meant
//! to be used in development and testing.

use std::borrow::Cow;

use anyhow::anyhow;
use async_trait::async_trait;
use http::Uri;
use merino_settings::Settings;

use crate::{SetupError, SuggestError, Suggestion, SuggestionProvider};

/// A toy suggester to test the system.
pub struct WikiFruit;

#[async_trait]
impl<'a> SuggestionProvider<'a> for WikiFruit {
    fn name(&self) -> Cow<'a, str> {
        Cow::from("WikiFruit")
    }

    async fn setup(&mut self, settings: &Settings) -> Result<(), SetupError> {
        if !settings.debug {
            Err(SetupError::InvalidConfiguration(anyhow!(
                "WikiFruit suggestion provider can only be used in debug mode",
            )))
        } else {
            Ok(())
        }
    }

    async fn suggest(&self, query: &str) -> Result<Vec<Suggestion>, SuggestError> {
        let suggestion = match query {
            "apple" => Some(Suggestion {
                id: 1,
                full_keyword: "apple".to_string(),
                title: "Wikipedia - Apple".to_string(),
                url: Uri::from_static("https://en.wikipedia.org/wiki/Apple"),
                impression_url: Uri::from_static("https://127.0.0.1/"),
                click_url: Uri::from_static("https://127.0.0.1/"),
                advertiser: "Merino::WikiFruit".to_string(),
                is_sponsored: false,
                icon: Uri::from_static("https://en.wikipedia.org/favicon.ico"),
            }),
            "banana" => Some(Suggestion {
                id: 1,
                full_keyword: "banana".to_string(),
                title: "Wikipedia - Banana".to_string(),
                url: Uri::from_static("https://en.wikipedia.org/wiki/Banana"),
                impression_url: Uri::from_static("https://127.0.0.1/"),
                click_url: Uri::from_static("https://127.0.0.1/"),
                advertiser: "Merino::WikiFruit".to_string(),
                is_sponsored: false,
                icon: Uri::from_static("https://en.wikipedia.org/favicon.ico"),
            }),
            "cherry" => Some(Suggestion {
                id: 1,
                full_keyword: "cherry".to_string(),
                title: "Wikipedia - Cherry".to_string(),
                url: Uri::from_static("https://en.wikipedia.org/wiki/Cherry"),
                impression_url: Uri::from_static("https://127.0.0.1/"),
                click_url: Uri::from_static("https://127.0.0.1/"),
                advertiser: "Merino::WikiFruit".to_string(),
                is_sponsored: false,
                icon: Uri::from_static("https://en.wikipedia.org/favicon.ico"),
            }),
            _ => None,
        };

        Ok(suggestion.into_iter().collect())
    }
}