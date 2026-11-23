use std::{collections::BTreeMap, rc::Rc};

use serde_json::Value;

use super::{
    Basis, Invocation, ProviderCatalog, ProviderCommand, ProviderDescription, ProviderDriver,
    ProviderMetadata, Records, Session, SystemDefinition, error,
};
use crate::Diagnostic;

pub trait ProviderSession {
    fn inspect(&self) -> Option<Value> {
        None
    }
    fn invoke(&mut self, invocation: &Invocation) -> Result<Records, Diagnostic>;
    fn cancel(&mut self, request_id: &str) -> Result<(), Diagnostic>;
    fn close(&mut self) -> Result<(), Diagnostic> {
        Ok(())
    }
}

pub trait Provider {
    fn contracts(&self) -> &[String];
    fn create(&self, config: &Value) -> Result<Box<dyn ProviderSession>, Diagnostic>;
}

#[derive(Default)]
pub struct ProviderRegistry {
    providers: BTreeMap<String, Rc<dyn Provider>>,
    catalog: ProviderCatalog,
}

impl ProviderRegistry {
    pub fn register(
        &mut self,
        id: impl Into<String>,
        provider: impl Provider + 'static,
    ) -> Result<(), Diagnostic> {
        self.register_with_metadata(id, provider, ProviderMetadata::default())
    }

    pub fn register_with_metadata(
        &mut self,
        id: impl Into<String>,
        provider: impl Provider + 'static,
        metadata: ProviderMetadata,
    ) -> Result<(), Diagnostic> {
        let id = id.into();
        self.catalog.register(ProviderDescription::new(
            id.clone(),
            provider.contracts().to_vec(),
            metadata,
        )?)?;
        self.providers.insert(id, Rc::new(provider));
        Ok(())
    }

    pub fn catalog(&self) -> &ProviderCatalog {
        &self.catalog
    }

    pub fn start(&self, definition: SystemDefinition, basis: Basis) -> Result<Runtime, Diagnostic> {
        let providers = self.catalog.inspect(None).into_iter().cloned().collect();
        Ok(Runtime {
            driver: ProviderDriver::new(definition, basis, providers)?,
            factories: self.providers.clone(),
            providers: BTreeMap::new(),
        })
    }
}

pub struct Runtime {
    driver: ProviderDriver,
    factories: BTreeMap<String, Rc<dyn Provider>>,
    providers: BTreeMap<String, Box<dyn ProviderSession>>,
}

pub type BasisReader<'a> = dyn FnMut() -> Result<Basis, Diagnostic> + 'a;
pub type OutputValidator<'a> = dyn FnMut(&Records, &Invocation) -> Result<(), Diagnostic> + 'a;

#[derive(Default)]
pub struct ProviderHooks<'a> {
    pub current_basis: Option<&'a mut BasisReader<'a>>,
    pub validate_outputs: Option<&'a mut OutputValidator<'a>>,
}

impl Runtime {
    pub fn provider_status(&self) -> Vec<Value> {
        self.providers
            .iter()
            .filter_map(|(block, provider)| {
                provider
                    .inspect()
                    .map(|status| serde_json::json!({"block":block,"status":status}))
            })
            .collect()
    }

    pub fn session(&self) -> &Session {
        self.driver.session()
    }
    pub fn session_mut(&mut self) -> &mut Session {
        self.driver.session_mut()
    }

    pub fn request(&mut self, source: &str) -> Result<String, Diagnostic> {
        self.driver.request(source)
    }

    pub fn invoke(
        &mut self,
        block: &str,
        request_id: &str,
        inputs: Records,
        allow_effects: bool,
    ) -> Result<Records, Diagnostic> {
        self.invoke_with(
            block,
            request_id,
            inputs,
            allow_effects,
            ProviderHooks::default(),
        )
    }

    pub fn invoke_with(
        &mut self,
        block: &str,
        request_id: &str,
        inputs: Records,
        allow_effects: bool,
        mut hooks: ProviderHooks<'_>,
    ) -> Result<Records, Diagnostic> {
        let command = self.driver.invoke(
            block,
            request_id,
            inputs,
            allow_effects,
            hooks.current_basis.is_some(),
            hooks.validate_outputs.is_some(),
        )?;
        let value = self.run(command, &mut hooks)?;
        Ok(serde_json::from_value(value).expect("provider outputs"))
    }

    pub fn cancel(&mut self, block: &str, request_id: &str) -> Result<(), Diagnostic> {
        if !self
            .session()
            .pending(request_id)
            .is_some_and(|call| call.block == block)
        {
            return Err(error("unknown_request", request_id));
        }
        let command = self.driver.cancel(request_id)?;
        self.run(command, &mut ProviderHooks::default())?;
        Ok(())
    }

    pub fn close(&mut self) -> Result<(), Diagnostic> {
        let command = self.driver.close()?;
        self.run(command, &mut ProviderHooks::default())?;
        Ok(())
    }

    fn run(
        &mut self,
        mut command: ProviderCommand,
        hooks: &mut ProviderHooks<'_>,
    ) -> Result<Value, Diagnostic> {
        loop {
            let (token, result, cleanup) = match command {
                ProviderCommand::Create { token, invocation } => {
                    let result = self.factories[&invocation.provider]
                        .create(&invocation.config)
                        .map(|provider| {
                            self.providers.insert(invocation.block, provider);
                            Value::Null
                        });
                    (token, result, false)
                }
                ProviderCommand::Invoke { token, invocation } => {
                    let result = self
                        .providers
                        .get_mut(&invocation.block)
                        .expect("created provider")
                        .invoke(&invocation)
                        .map(|outputs| serde_json::to_value(outputs).expect("provider outputs"));
                    (token, result, false)
                }
                ProviderCommand::CurrentBasis { token, .. } => {
                    let result = hooks.current_basis.as_mut().expect("basis callback")()
                        .map(|basis| serde_json::to_value(basis).expect("provider basis"));
                    (token, result, false)
                }
                ProviderCommand::ValidateOutputs {
                    token,
                    invocation,
                    outputs,
                } => {
                    let result = hooks.validate_outputs.as_mut().expect("output validator")(
                        &outputs,
                        &invocation,
                    )
                    .map(|()| Value::Null);
                    (token, result, false)
                }
                ProviderCommand::Cancel {
                    token,
                    block,
                    request_id,
                } => {
                    let result = self
                        .providers
                        .get_mut(&block)
                        .expect("created provider")
                        .cancel(&request_id)
                        .map(|()| Value::Bool(true));
                    (token, result, false)
                }
                ProviderCommand::Close { token, block } => {
                    let result = self
                        .providers
                        .remove(&block)
                        .expect("created provider")
                        .close()
                        .map(|()| Value::Null);
                    (token, result, true)
                }
                ProviderCommand::Done { value } => return Ok(value),
            };
            command = match result {
                Ok(value) => self.driver.resume(token, value, None)?,
                Err(cause) => {
                    let next = self
                        .driver
                        .resume(token, Value::Null, Some(cause.to_string()));
                    if !cleanup {
                        return Err(cause);
                    }
                    next?
                }
            };
        }
    }
}
