//! A suggestion provider that filters suggestions from a subprovider.

use std::collections::HashMap;

use crate::{
    CacheInputs, SetupError, SuggestError, SuggestionProvider, SuggestionRequest,
    SuggestionResponse,
};
use anyhow::Context;
use async_trait::async_trait;
use blake3::Hash;
use cadence::{Counted, StatsdClient};
use regex::{RegexSet, RegexSetBuilder};

/// A combinator provider that filters the results from the wrapped provider
/// using a blocklist from the settings.
pub struct KeywordFilterProvider {
    /// The list of ids for the blocklist rules.
    blocklist_ids: Vec<String>,

    /// The regex set containing the blocklist rules. Items have
    /// the same sorting order as `blocklist_ids`.
    blocklist_rules: RegexSet,

    /// A hash of all of the rules in this filter, for cache key determination.
    blocklist_hash: Hash,

    /// The provider to pull suggestions from.
    inner: Box<dyn SuggestionProvider>,

    /// The Statsd client used to record statistics.
    metrics_client: StatsdClient,
}

impl KeywordFilterProvider {
    /// Construct a new, boxed filter provider.
    pub fn new_boxed(
        blocklist: HashMap<String, String>,
        inner: Box<dyn SuggestionProvider>,
        metrics_client: &StatsdClient,
    ) -> Result<Box<Self>, SetupError> {
        let (blocklist_ids, regexes): (Vec<String>, Vec<String>) = blocklist.into_iter().unzip();

        let mut hasher = blake3::Hasher::new();
        for r in &regexes {
            hasher.update(r.as_bytes());
        }
        let blocklist_hash = hasher.finalize();

        let blocklist_rules = RegexSetBuilder::new(regexes)
            .case_insensitive(true)
            .build()
            .context("KeywordFilterProvider failed to compile the regex set.")
            .map_err(SetupError::InvalidConfiguration)?;

        Ok(Box::new(Self {
            blocklist_ids,
            blocklist_rules,
            blocklist_hash,
            inner,
            metrics_client: metrics_client.clone(),
        }))
    }
}

#[async_trait]
impl SuggestionProvider for KeywordFilterProvider {
    fn name(&self) -> String {
        format!("KeywordFilterProvider({})", self.inner.name())
    }

    fn cache_inputs(&self, req: &SuggestionRequest, cache_inputs: &mut dyn CacheInputs) {
        cache_inputs.add(self.blocklist_hash.as_bytes());
        self.inner.cache_inputs(req, cache_inputs);
    }

    async fn suggest(&self, query: SuggestionRequest) -> Result<SuggestionResponse, SuggestError> {
        let mut results = self
            .inner
            .suggest(query)
            .await
            .unwrap_or_else(|_| SuggestionResponse::new(vec![]));

        let mut reported_hits: HashMap<String, i64> = HashMap::new();
        results.suggestions.retain(|r| {
            let matches: Vec<_> = self.blocklist_rules.matches(&r.title).into_iter().collect();

            for rule_index in &matches {
                // The following unwrap is safe: the regex set and blockist ids
                // were generated at the same time from the same configuration.
                let rule_id = self.blocklist_ids.get(*rule_index).unwrap();
                if let Some(k) = reported_hits.get_mut(rule_id) {
                    *k += 1;
                } else {
                    reported_hits.insert(rule_id.to_string(), 1);
                }
            }

            matches.is_empty()
        });

        for (id, value) in reported_hits {
            self.metrics_client
                // Note: the i64 conversion is required because `ToCounterValue` is
                // not implemented for `usize`.
                .count_with_tags("keywordfilter.match", value)
                .with_tag("id", &id)
                .try_send()
                .ok();
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        CacheStatus, KeywordFilterProvider, SuggestError, Suggestion, SuggestionProvider,
        SuggestionRequest, SuggestionResponse,
    };
    use async_trait::async_trait;
    use cadence::{SpyMetricSink, StatsdClient};
    use fake::{Fake, Faker};
    use std::collections::HashMap;

    struct TestSuggestionsProvider();

    #[async_trait]
    impl SuggestionProvider for TestSuggestionsProvider {
        fn name(&self) -> String {
            "TestSuggestionsProvider()".to_string()
        }

        async fn suggest(
            &self,
            _query: SuggestionRequest,
        ) -> Result<SuggestionResponse, SuggestError> {
            Ok(SuggestionResponse {
                cache_status: CacheStatus::NoCache,
                cache_ttl: None,
                suggestions: vec![
                    Suggestion {
                        provider: self.name(),
                        title: "A test title".to_string(),
                        full_keyword: "test".to_string(),
                        ..Faker.fake()
                    },
                    Suggestion {
                        provider: self.name(),
                        title: "A suggestion that goes through".to_string(),
                        full_keyword: "not matched".to_string(),
                        ..Faker.fake()
                    },
                    Suggestion {
                        provider: self.name(),
                        title: "Another suggestion that goes through".to_string(),
                        full_keyword: "not matched".to_string(),
                        ..Faker.fake()
                    },
                ],
            })
        }
    }

    #[tokio::test]
    async fn test_provider_filters() {
        let mut blocklist = HashMap::new();
        blocklist.insert("filter_1".to_string(), "test".to_string());

        let (rx, sink) = SpyMetricSink::new();
        let metrics_client = StatsdClient::from_sink("merino-test", sink);

        let filter_provider = KeywordFilterProvider::new_boxed(
            blocklist,
            Box::new(TestSuggestionsProvider()),
            &metrics_client,
        )
        .expect("failed to create the keyword filter provider");

        let res = filter_provider
            .suggest(Faker.fake())
            .await
            .expect("failed to get suggestion");

        assert_eq!(res.suggestions.len(), 2);
        assert_eq!(res.suggestions[0].provider, "TestSuggestionsProvider()");
        assert_eq!(res.suggestions[0].title, "A suggestion that goes through");

        // Verify that the filtering was properly recorded.
        assert_eq!(rx.len(), 1);
        let sent = rx.recv().unwrap();
        assert_eq!(
            "merino-test.keywordfilter.match:1|c|#id:filter_1",
            String::from_utf8(sent).unwrap()
        );
    }

    #[tokio::test]
    async fn test_provider_all_filtered() {
        let mut blocklist = HashMap::new();
        blocklist.insert("filter_1".to_string(), "test".to_string());
        blocklist.insert("filter_2".to_string(), "through".to_string());

        let (rx, sink) = SpyMetricSink::new();
        let metrics_client = StatsdClient::from_sink("merino-test", sink);

        let filter_provider = KeywordFilterProvider::new_boxed(
            blocklist,
            Box::new(TestSuggestionsProvider()),
            &metrics_client,
        )
        .expect("failed to create the keyword filter provider");

        let res = filter_provider
            .suggest(Faker.fake())
            .await
            .expect("failed to get suggestion");

        assert_eq!(res.suggestions.len(), 0);

        // Verify that the filtering was properly recorded.
        assert_eq!(rx.len(), 2);
        let collected_data: Vec<String> = rx
            .iter()
            .take(2)
            .map(|x| String::from_utf8(x).unwrap())
            .collect();
        assert!(collected_data
            .contains(&"merino-test.keywordfilter.match:1|c|#id:filter_1".to_string()));
        assert!(collected_data
            .contains(&"merino-test.keywordfilter.match:2|c|#id:filter_2".to_string()));
    }

    #[tokio::test]
    async fn test_provider_nothing_filtered() {
        let mut blocklist = HashMap::new();
        blocklist.insert("filter_1".to_string(), "no-match".to_string());

        let (rx, sink) = SpyMetricSink::new();
        let metrics_client = StatsdClient::from_sink("merino-test", sink);

        let filter_provider = KeywordFilterProvider::new_boxed(
            blocklist,
            Box::new(TestSuggestionsProvider()),
            &metrics_client,
        )
        .expect("failed to create the keyword filter provider");

        let res = filter_provider
            .suggest(Faker.fake())
            .await
            .expect("failed to get suggestion");

        assert_eq!(res.suggestions.len(), 3);

        // Verify that nothing was recorded.
        assert!(rx.is_empty());
    }
}
