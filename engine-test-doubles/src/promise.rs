use aurora_engine_sdk::promise::PromiseHandler;
use aurora_engine_sdk::promise::PromiseId;
use aurora_engine_types::parameters::{
    NearPromise, PromiseBatchAction, PromiseCreateArgs, SimpleNearPromise,
};
use aurora_engine_types::types::PromiseResult;
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq)]
pub enum PromiseArgs {
    Create(PromiseCreateArgs),
    #[allow(dead_code)]
    Callback {
        base: PromiseId,
        callback: PromiseCreateArgs,
    },
    Batch(PromiseBatchAction),
    Recursive(NearPromise),
}

/// Doesn't actually schedule any promises, only tracks what promises should be scheduled
#[derive(Default)]
pub struct PromiseTracker {
    internal_index: u64,
    pub promise_results: Vec<PromiseResult>,
    pub scheduled_promises: HashMap<u64, PromiseArgs>,
    pub returned_promise: Option<PromiseId>,
}

impl PromiseTracker {
    fn take_id(&mut self) -> u64 {
        let id = self.internal_index;
        self.internal_index += 1;
        id
    }

    fn remove_as_near_promise(&mut self, id: u64) -> Option<NearPromise> {
        let result = match self.scheduled_promises.remove(&id)? {
            PromiseArgs::Batch(x) => NearPromise::Simple(SimpleNearPromise::Batch(x)),
            PromiseArgs::Create(x) => NearPromise::Simple(SimpleNearPromise::Create(x)),
            PromiseArgs::Recursive(x) => x,
            PromiseArgs::Callback { base, callback } => {
                let base_promise = self.remove_as_near_promise(base.raw())?;
                NearPromise::Then {
                    base: Box::new(base_promise),
                    callback: SimpleNearPromise::Create(callback),
                }
            }
        };
        Some(result)
    }
}

impl PromiseHandler for PromiseTracker {
    type ReadOnly = Self;

    fn promise_results_count(&self) -> u64 {
        u64::try_from(self.promise_results.len()).unwrap_or_default()
    }

    fn promise_result(&self, index: u64) -> Option<PromiseResult> {
        self.promise_results
            .get(usize::try_from(index).ok()?)
            .cloned()
    }

    unsafe fn promise_create_call(&mut self, args: &PromiseCreateArgs) -> PromiseId {
        let id = self.take_id();
        self.scheduled_promises
            .insert(id, PromiseArgs::Create(args.clone()));
        PromiseId::new(id)
    }

    unsafe fn promise_create_and_combine(&mut self, args: &[PromiseCreateArgs]) -> PromiseId {
        let id = self.take_id();
        self.scheduled_promises.insert(
            id,
            PromiseArgs::Recursive(NearPromise::And(
                args.iter()
                    .map(|p| NearPromise::Simple(SimpleNearPromise::Create(p.clone())))
                    .collect(),
            )),
        );
        PromiseId::new(id)
    }

    unsafe fn promise_attach_callback(
        &mut self,
        base: PromiseId,
        callback: &PromiseCreateArgs,
    ) -> PromiseId {
        let id = self.take_id();
        self.scheduled_promises.insert(
            id,
            PromiseArgs::Callback {
                base,
                callback: callback.clone(),
            },
        );
        PromiseId::new(id)
    }

    unsafe fn promise_create_batch(&mut self, args: &PromiseBatchAction) -> PromiseId {
        let id = self.take_id();
        self.scheduled_promises
            .insert(id, PromiseArgs::Batch(args.clone()));
        PromiseId::new(id)
    }

    unsafe fn promise_attach_batch_callback(
        &mut self,
        base: PromiseId,
        args: &PromiseBatchAction,
    ) -> PromiseId {
        let id = self.take_id();
        let base_promise = self
            .remove_as_near_promise(base.raw())
            .expect("Base promise id must be known");
        let new_promise = PromiseArgs::Recursive(NearPromise::Then {
            base: Box::new(base_promise),
            callback: SimpleNearPromise::Batch(args.clone()),
        });
        self.scheduled_promises.insert(id, new_promise);
        PromiseId::new(id)
    }

    fn promise_return(&mut self, promise: PromiseId) {
        self.returned_promise = Some(promise);
    }

    fn read_only(&self) -> Self::ReadOnly {
        Self {
            internal_index: 0,
            promise_results: self.promise_results.clone(),
            scheduled_promises: HashMap::default(),
            returned_promise: Option::default(),
        }
    }
}
