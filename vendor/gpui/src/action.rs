use anyhow::{Context as _, Result};
use collections::HashMap;
pub use gpui_macros::Action;
pub use no_action::{NoAction, is_no_action};
use serde_json::json;
use std::{
    any::{Any, TypeId},
    fmt::Display,
};

#[macro_export]
macro_rules! actions {
    ($namespace:path, [ $( $(#[$attr:meta])* $name:ident),* $(,)? ]) => {
        $(
            #[derive(::std::clone::Clone, ::std::cmp::PartialEq, ::std::default::Default, ::std::fmt::Debug, gpui::Action)]
            #[action(namespace = $namespace)]
            $(#[$attr])*
            pub struct $name;
        )*
    };
    ([ $( $(#[$attr:meta])* $name:ident),* $(,)? ]) => {
        $(
            #[derive(::std::clone::Clone, ::std::cmp::PartialEq, ::std::default::Default, ::std::fmt::Debug, gpui::Action)]
            $(#[$attr])*
            pub struct $name;
        )*
    };
}

pub trait Action: Any + Send {
    fn boxed_clone(&self) -> Box<dyn Action>;

    fn partial_eq(&self, action: &dyn Action) -> bool;

    fn name(&self) -> &'static str;

    fn name_for_type() -> &'static str
    where
        Self: Sized;

    fn build(value: serde_json::Value) -> Result<Box<dyn Action>>
    where
        Self: Sized;

    fn action_json_schema(_: &mut schemars::SchemaGenerator) -> Option<schemars::Schema>
    where
        Self: Sized,
    {
        None
    }

    fn deprecated_aliases() -> &'static [&'static str]
    where
        Self: Sized,
    {
        &[]
    }

    fn deprecation_message() -> Option<&'static str>
    where
        Self: Sized,
    {
        None
    }

    fn documentation() -> Option<&'static str>
    where
        Self: Sized,
    {
        None
    }
}

impl std::fmt::Debug for dyn Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("dyn Action")
            .field("name", &self.name())
            .finish()
    }
}

impl dyn Action {
    pub fn as_any(&self) -> &dyn Any {
        self as &dyn Any
    }
}

#[derive(Debug)]
pub enum ActionBuildError {
    NotFound {
        name: String,
    },
    BuildError {
        name: String,
        error: anyhow::Error,
    },
}

impl std::error::Error for ActionBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ActionBuildError::NotFound { .. } => None,
            ActionBuildError::BuildError { error, .. } => error.source(),
        }
    }
}

impl Display for ActionBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionBuildError::NotFound { name } => {
                write!(f, "Didn't find an action named \"{name}\"")
            }
            ActionBuildError::BuildError { name, error } => {
                write!(f, "Error while building action \"{name}\": {error}")
            }
        }
    }
}

type ActionBuilder = fn(json: serde_json::Value) -> anyhow::Result<Box<dyn Action>>;

pub(crate) struct ActionRegistry {
    by_name: HashMap<&'static str, ActionData>,
    names_by_type_id: HashMap<TypeId, &'static str>,
    all_names: Vec<&'static str>,
    deprecated_aliases: HashMap<&'static str, &'static str>,
    deprecation_messages: HashMap<&'static str, &'static str>,
    documentation: HashMap<&'static str, &'static str>,
}

impl Default for ActionRegistry {
    fn default() -> Self {
        let mut this = ActionRegistry {
            by_name: Default::default(),
            names_by_type_id: Default::default(),
            documentation: Default::default(),
            all_names: Default::default(),
            deprecated_aliases: Default::default(),
            deprecation_messages: Default::default(),
        };

        this.load_actions();

        this
    }
}

struct ActionData {
    pub build: ActionBuilder,
    pub json_schema: fn(&mut schemars::SchemaGenerator) -> Option<schemars::Schema>,
}

#[doc(hidden)]
pub struct MacroActionBuilder(pub fn() -> MacroActionData);

#[doc(hidden)]
pub struct MacroActionData {
    pub name: &'static str,
    pub type_id: TypeId,
    pub build: ActionBuilder,
    pub json_schema: fn(&mut schemars::SchemaGenerator) -> Option<schemars::Schema>,
    pub deprecated_aliases: &'static [&'static str],
    pub deprecation_message: Option<&'static str>,
    pub documentation: Option<&'static str>,
}

inventory::collect!(MacroActionBuilder);

impl ActionRegistry {
    pub(crate) fn load_actions(&mut self) {
        for builder in inventory::iter::<MacroActionBuilder> {
            let action = builder.0();
            self.insert_action(action);
        }
    }

    #[cfg(test)]
    pub(crate) fn load_action<A: Action>(&mut self) {
        self.insert_action(MacroActionData {
            name: A::name_for_type(),
            type_id: TypeId::of::<A>(),
            build: A::build,
            json_schema: A::action_json_schema,
            deprecated_aliases: A::deprecated_aliases(),
            deprecation_message: A::deprecation_message(),
            documentation: A::documentation(),
        });
    }

    fn insert_action(&mut self, action: MacroActionData) {
        let name = action.name;
        if self.by_name.contains_key(name) {
            panic!(
                "Action with name `{name}` already registered \
                (might be registered in `#[action(deprecated_aliases = [...])]`."
            );
        }
        self.by_name.insert(
            name,
            ActionData {
                build: action.build,
                json_schema: action.json_schema,
            },
        );
        for &alias in action.deprecated_aliases {
            if self.by_name.contains_key(alias) {
                panic!(
                    "Action with name `{alias}` already registered. \
                    `{alias}` is specified in `#[action(deprecated_aliases = [...])]` for action `{name}`."
                );
            }
            self.by_name.insert(
                alias,
                ActionData {
                    build: action.build,
                    json_schema: action.json_schema,
                },
            );
            self.deprecated_aliases.insert(alias, name);
            self.all_names.push(alias);
        }
        self.names_by_type_id.insert(action.type_id, name);
        self.all_names.push(name);
        if let Some(deprecation_msg) = action.deprecation_message {
            self.deprecation_messages.insert(name, deprecation_msg);
        }
        if let Some(documentation) = action.documentation {
            self.documentation.insert(name, documentation);
        }
    }

    pub fn build_action_type(&self, type_id: &TypeId) -> Result<Box<dyn Action>> {
        let name = self
            .names_by_type_id
            .get(type_id)
            .with_context(|| format!("no action type registered for {type_id:?}"))?;

        Ok(self.build_action(name, None)?)
    }

    pub fn build_action(
        &self,
        name: &str,
        params: Option<serde_json::Value>,
    ) -> std::result::Result<Box<dyn Action>, ActionBuildError> {
        let build_action = self
            .by_name
            .get(name)
            .ok_or_else(|| ActionBuildError::NotFound {
                name: name.to_owned(),
            })?
            .build;
        (build_action)(params.unwrap_or_else(|| json!({}))).map_err(|e| {
            ActionBuildError::BuildError {
                name: name.to_owned(),
                error: e,
            }
        })
    }

    pub fn all_action_names(&self) -> &[&'static str] {
        self.all_names.as_slice()
    }

    pub fn action_schemas(
        &self,
        generator: &mut schemars::SchemaGenerator,
    ) -> Vec<(&'static str, Option<schemars::Schema>)> {
        self.all_names
            .iter()
            .map(|name| {
                let action_data = self
                    .by_name
                    .get(name)
                    .expect("All actions in all_names should be registered");
                (*name, (action_data.json_schema)(generator))
            })
            .collect::<Vec<_>>()
    }

    pub fn deprecated_aliases(&self) -> &HashMap<&'static str, &'static str> {
        &self.deprecated_aliases
    }

    pub fn deprecation_messages(&self) -> &HashMap<&'static str, &'static str> {
        &self.deprecation_messages
    }

    pub fn documentation(&self) -> &HashMap<&'static str, &'static str> {
        &self.documentation
    }
}

pub fn generate_list_of_all_registered_actions() -> impl Iterator<Item = MacroActionData> {
    inventory::iter::<MacroActionBuilder>
        .into_iter()
        .map(|builder| builder.0())
}

mod no_action {
    use crate as gpui;
    use std::any::Any as _;

    actions!(
        zed,
        [
            #[doc = " Action with special handling which unbinds the keybinding this is associated with,"]
            #[doc = " if it is the highest precedence match."]
            NoAction
        ]
    );

    pub fn is_no_action(action: &dyn gpui::Action) -> bool {
        action.as_any().type_id() == (NoAction {}).type_id()
    }
}
