//! A provider that executes an inner provider, but returns no suggestions.

use crate::{CacheInputs, SuggestError, SuggestionProvider, SuggestionRequest, SuggestionResponse};
use async_trait::async_trait;

/// A provider that runs `inner`, but doesn't return any results.
pub struct StealthProvider {
    /// The provider to run but not provide suggestions from.
    inner: Box<dyn SuggestionProvider>,
}

#[async_trait]
impl SuggestionProvider for StealthProvider {
    fn name(&self) -> String {
        format!("stealth({})", self.inner.name())
    }

    fn cache_inputs(&self, req: &SuggestionRequest, cache_inputs: &mut dyn CacheInputs) {
        self.inner.cache_inputs(req, cache_inputs);
    }

    async fn suggest(
        &self,
        request: SuggestionRequest,
    ) -> Result<SuggestionResponse, SuggestError> {
        self.inner.suggest(request).await?;
        Ok(SuggestionResponse::new(vec![]))
    }
}

impl StealthProvider {
    /// Make a new stealth provider, wrapping the given provider.
    #[must_use]
    pub fn new_boxed(inner: Box<dyn SuggestionProvider>) -> Box<Self> {
        Box::new(Self { inner })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        StealthProvider, SuggestError, Suggestion, SuggestionProvider, SuggestionRequest,
        SuggestionResponse,
    };
    use async_trait::async_trait;
    use fake::{Fake, Faker};
    use futures::StreamExt;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct CounterProvider {
        counter: AtomicU32,
    }

    #[async_trait]
    impl SuggestionProvider for CounterProvider {
        fn name(&self) -> String {
            "CounterProvider".to_string()
        }

        async fn suggest(
            &self,
            _query: SuggestionRequest,
        ) -> Result<SuggestionResponse, SuggestError> {
            self.counter.fetch_add(1, Ordering::SeqCst);
            Ok(SuggestionResponse::new(vec![Suggestion {
                title: format!("{}", self.counter.load(Ordering::SeqCst)),
                ..Faker.fake()
            }]))
        }
    }

    #[tokio::test]
    async fn test_stress() {
        let counter = Box::new(CounterProvider {
            counter: AtomicU32::new(0),
        });
        let stealth = StealthProvider { inner: counter };

        // We want this to run as parallel as possible
        let mut futures = futures::stream::FuturesUnordered::new();
        for _ in 0..100 {
            futures.push(async {
                let res = stealth.suggest(Faker.fake()).await.unwrap();
                assert!(res.suggestions.is_empty());
            });
        }

        while futures.next().await.is_some() {}

        // Ask the counter provider how many times it was called.
        let res = stealth.inner.suggest(Faker.fake()).await.unwrap();
        let count: u32 = res.suggestions[0].title.parse().unwrap();
        // 100 from the loop above, and another from right now.
        assert_eq!(count, 101);
    }
}
