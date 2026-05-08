use std::borrow::Cow;

use inflection::{plural, singular};
use maud::{Markup, Render, html};
use rusqlite::types::Value;
use seekwel::model::{Column, ColumnDef, Model, ModelRecord};

pub use maud;

pub fn form_for<M>(model: &M) -> FormFor<'_, M>
where
    M: ModelRecord,
{
    FormFor::new(model)
}

pub fn form_for_persisted<M>(model: &M) -> FormFor<'_, M>
where
    M: ModelRecord,
{
    form_for(model)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl FormMethod {
    fn form_method(self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::Post | Self::Put | Self::Patch | Self::Delete => "post",
        }
    }

    fn method_override(self) -> Option<&'static str> {
        match self {
            Self::Get | Self::Post => None,
            Self::Put => Some("put"),
            Self::Patch => Some("patch"),
            Self::Delete => Some("delete"),
        }
    }
}

impl From<&str> for FormMethod {
    fn from(method: &str) -> Self {
        match method.to_ascii_lowercase().as_str() {
            "get" => Self::Get,
            "put" => Self::Put,
            "patch" => Self::Patch,
            "delete" => Self::Delete,
            _ => Self::Post,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl FieldValue {
    pub fn as_input_value(&self) -> Option<String> {
        match self {
            Self::Null | Self::Blob(_) => None,
            Self::Integer(value) => Some(value.to_string()),
            Self::Real(value) => Some(value.to_string()),
            Self::Text(value) => Some(value.clone()),
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Null | Self::Blob(_) => false,
            Self::Integer(value) => *value != 0,
            Self::Real(value) => *value != 0.0,
            Self::Text(value) => matches!(value.as_str(), "true" | "t" | "1" | "yes" | "on"),
        }
    }
}

impl From<Value> for FieldValue {
    fn from(value: Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Integer(value) => Self::Integer(value),
            Value::Real(value) => Self::Real(value),
            Value::Text(value) => Self::Text(value),
            Value::Blob(value) => Self::Blob(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormField {
    pub name: &'static str,
    pub sql_type: &'static str,
    pub nullable: bool,
    pub value: FieldValue,
}

#[derive(Debug, Clone)]
pub struct FormFor<'a, M>
where
    M: ModelRecord,
{
    model: &'a M,
    action: Option<String>,
    method: Option<FormMethod>,
    id: Option<String>,
    class: Option<String>,
    param_name: Option<String>,
    submit_label: Option<String>,
    include_submit: bool,
}

impl<'a, M> FormFor<'a, M>
where
    M: ModelRecord,
{
    pub fn new(model: &'a M) -> Self {
        Self {
            model,
            action: None,
            method: None,
            id: None,
            class: None,
            param_name: None,
            submit_label: None,
            include_submit: true,
        }
    }

    pub fn action(mut self, action: impl Into<String>) -> Self {
        self.action = Some(action.into());
        self
    }

    pub fn method(mut self, method: impl Into<FormMethod>) -> Self {
        self.method = Some(method.into());
        self
    }

    pub fn get(self) -> Self {
        self.method(FormMethod::Get)
    }

    pub fn post(self) -> Self {
        self.method(FormMethod::Post)
    }

    pub fn put(self) -> Self {
        self.method(FormMethod::Put)
    }

    pub fn patch(self) -> Self {
        self.method(FormMethod::Patch)
    }

    pub fn delete(self) -> Self {
        self.method(FormMethod::Delete)
    }

    pub fn persisted(self) -> Self {
        self.patch()
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn class(mut self, class: impl Into<String>) -> Self {
        self.class = Some(class.into());
        self
    }

    pub fn param_name(mut self, param_name: impl Into<String>) -> Self {
        self.param_name = Some(param_name.into());
        self
    }

    pub fn submit_label(mut self, label: impl Into<String>) -> Self {
        self.submit_label = Some(label.into());
        self
    }

    pub fn without_submit(mut self) -> Self {
        self.include_submit = false;
        self
    }

    pub fn fields<F>(self, fields: F) -> Markup
    where
        F: FnOnce(&FormBuilder<'_, M>) -> Markup,
    {
        let builder = self.builder();
        let body = fields(&builder);
        self.render_form(body)
    }

    pub fn builder(&self) -> FormBuilder<'_, M> {
        FormBuilder::new(self.model, self.resolved_param_name())
    }

    pub fn render_markup(&self) -> Markup {
        let builder = self.builder();
        let body = builder.default_fields(self.submit_label.as_deref(), self.include_submit);
        self.render_form(body)
    }

    fn render_form(&self, body: Markup) -> Markup {
        let action = self
            .action
            .clone()
            .unwrap_or_else(|| default_action(self.model));
        let method = self.resolved_method();
        let form_method = method.form_method();
        let override_method = method.method_override();

        html! {
            form action=(action) method=(form_method) id=[self.id.as_deref()] class=[self.class.as_deref()] {
                @if let Some(override_method) = override_method {
                    input type="hidden" name="_method" value=(override_method);
                }
                (body)
            }
        }
    }

    fn resolved_method(&self) -> FormMethod {
        self.method.unwrap_or_else(|| {
            if self.model.is_persisted() {
                FormMethod::Patch
            } else {
                FormMethod::Post
            }
        })
    }

    fn resolved_param_name(&self) -> String {
        self.param_name
            .clone()
            .unwrap_or_else(default_param_name::<M>)
    }
}

impl<M> Render for FormFor<'_, M>
where
    M: ModelRecord,
{
    fn render(&self) -> Markup {
        self.render_markup()
    }
}

#[derive(Debug, Clone)]
pub struct FormBuilder<'a, M>
where
    M: Model,
{
    model: &'a M,
    param_name: String,
    fields: Vec<FormField>,
}

impl<'a, M> FormBuilder<'a, M>
where
    M: Model,
{
    fn new(model: &'a M, param_name: String) -> Self {
        Self {
            model,
            param_name,
            fields: fields_for(model),
        }
    }

    pub fn model(&self) -> &'a M {
        self.model
    }

    pub fn param_name(&self) -> &str {
        &self.param_name
    }

    pub fn fields(&self) -> &[FormField] {
        &self.fields
    }

    pub fn field_name<C>(&self, column: C) -> String
    where
        C: Column,
    {
        self.field_name_named(column.as_str())
    }

    pub fn field_name_named(&self, column_name: impl AsRef<str>) -> String {
        format!("{}[{}]", self.param_name, column_name.as_ref())
    }

    pub fn field_id<C>(&self, column: C) -> String
    where
        C: Column,
    {
        self.field_id_named(column.as_str())
    }

    pub fn field_id_named(&self, column_name: impl AsRef<str>) -> String {
        format!(
            "{}_{}",
            sanitize_id_part(&self.param_name),
            sanitize_id_part(column_name.as_ref())
        )
    }

    pub fn value<C>(&self, column: C) -> Option<&FieldValue>
    where
        C: Column,
    {
        self.value_named(column.as_str())
    }

    pub fn value_named(&self, column_name: impl AsRef<str>) -> Option<&FieldValue> {
        let column_name = column_name.as_ref();
        self.fields
            .iter()
            .find(|field| field.name == column_name)
            .map(|field| &field.value)
    }

    pub fn label<C>(&self, column: C, text: impl Render) -> Markup
    where
        C: Column,
    {
        self.label_named(column.as_str(), text)
    }

    pub fn label_named(&self, column_name: impl AsRef<str>, text: impl Render) -> Markup {
        let id = self.field_id_named(column_name);
        html! {
            label for=(id) { (text) }
        }
    }

    pub fn text_field<C>(&self, column: C) -> Markup
    where
        C: Column,
    {
        self.input("text", column)
    }

    pub fn text_field_named(&self, column_name: impl AsRef<str>) -> Markup {
        self.input_named("text", column_name)
    }

    pub fn number_field<C>(&self, column: C) -> Markup
    where
        C: Column,
    {
        self.input("number", column)
    }

    pub fn number_field_named(&self, column_name: impl AsRef<str>) -> Markup {
        self.input_named("number", column_name)
    }

    pub fn hidden_field<C>(&self, column: C) -> Markup
    where
        C: Column,
    {
        self.input("hidden", column)
    }

    pub fn hidden_field_named(&self, column_name: impl AsRef<str>) -> Markup {
        self.input_named("hidden", column_name)
    }

    pub fn textarea<C>(&self, column: C) -> Markup
    where
        C: Column,
    {
        self.textarea_named(column.as_str())
    }

    pub fn textarea_named(&self, column_name: impl AsRef<str>) -> Markup {
        let column_name = column_name.as_ref();
        let name = self.field_name_named(column_name);
        let id = self.field_id_named(column_name);
        let value = self
            .value_named(column_name)
            .and_then(FieldValue::as_input_value)
            .unwrap_or_default();

        html! {
            textarea name=(name) id=(id) { (value) }
        }
    }

    pub fn checkbox<C>(&self, column: C) -> Markup
    where
        C: Column,
    {
        self.checkbox_named(column.as_str())
    }

    pub fn checkbox_named(&self, column_name: impl AsRef<str>) -> Markup {
        let column_name = column_name.as_ref();
        let name = self.field_name_named(column_name);
        let id = self.field_id_named(column_name);
        let checked = self
            .value_named(column_name)
            .is_some_and(FieldValue::is_truthy);

        html! {
            input type="hidden" name=(name) value="0";
            input type="checkbox" name=(name) id=(id) value="1" checked[checked];
        }
    }

    pub fn submit(&self, label: impl Render) -> Markup {
        html! {
            input type="submit" value=(label);
        }
    }

    pub fn input<C>(&self, input_type: impl AsRef<str>, column: C) -> Markup
    where
        C: Column,
    {
        self.input_named(input_type, column.as_str())
    }

    pub fn input_named(&self, input_type: impl AsRef<str>, column_name: impl AsRef<str>) -> Markup {
        let column_name = column_name.as_ref();
        let name = self.field_name_named(column_name);
        let id = self.field_id_named(column_name);
        let value = self
            .value_named(column_name)
            .and_then(FieldValue::as_input_value);
        let input_type = input_type.as_ref().to_string();

        html! {
            input type=(input_type) name=(name) id=(id) value=[value];
        }
    }

    fn default_fields(&self, submit_label: Option<&str>, include_submit: bool) -> Markup {
        let submit_label = submit_label
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Owned(default_submit_label::<M>()));

        html! {
            @for field in &self.fields {
                div class="field" {
                    (self.label_named(field.name, human_label(field.name)))
                    (self.default_input(field))
                }
            }
            @if include_submit {
                (self.submit(submit_label.as_ref()))
            }
        }
    }

    fn default_input(&self, field: &FormField) -> Markup {
        match field.sql_type {
            "INTEGER" => self.number_field_named(field.name),
            "REAL" => {
                let column_name = field.name;
                let name = self.field_name_named(column_name);
                let id = self.field_id_named(column_name);
                let value = field.value.as_input_value();

                html! {
                    input type="number" step="any" name=(name) id=(id) value=[value];
                }
            }
            "BLOB" => {
                let name = self.field_name_named(field.name);
                let id = self.field_id_named(field.name);

                html! {
                    input type="file" name=(name) id=(id);
                }
            }
            _ => self.text_field_named(field.name),
        }
    }
}

fn fields_for<M>(model: &M) -> Vec<FormField>
where
    M: Model,
{
    M::columns()
        .iter()
        .zip(model.params())
        .map(|(column, value)| field_for(column, value))
        .collect()
}

fn field_for(column: &ColumnDef, value: Value) -> FormField {
    FormField {
        name: column.name,
        sql_type: column.sql_type,
        nullable: column.nullable,
        value: value.into(),
    }
}

fn default_action<M>(model: &M) -> String
where
    M: ModelRecord,
{
    match model.persisted_primary_key_value() {
        Some(value) => format!("/{}/{}", route_resource_name::<M>(), route_value(&value)),
        None => format!("/{}", route_resource_name::<M>()),
    }
}

fn route_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Integer(value) => value.to_string(),
        Value::Real(value) => value.to_string(),
        Value::Text(value) => percent_encode_path_segment(value),
        Value::Blob(value) => value.iter().map(|byte| format!("{byte:02x}")).collect(),
    }
}

fn route_resource_name<M>() -> String
where
    M: Model,
{
    let resource: String = plural(M::table_name());
    to_snake_case(&resource)
}

fn default_param_name<M>() -> String
where
    M: Model,
{
    let param: String = singular(M::table_name());
    to_snake_case(&param)
}

fn default_submit_label<M>() -> String
where
    M: Model,
{
    format!("Save {}", titleize(&default_param_name::<M>()))
}

fn human_label(column: &str) -> String {
    titleize(&column.replace('_', " "))
}

fn to_snake_case(value: &str) -> String {
    let mut output = String::new();
    let mut previous_was_separator = false;
    let mut previous_was_lower_or_digit = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && previous_was_lower_or_digit && !previous_was_separator {
                output.push('_');
            }
            output.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
            previous_was_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else if !output.is_empty() && !previous_was_separator {
            output.push('_');
            previous_was_separator = true;
            previous_was_lower_or_digit = false;
        }
    }

    while output.ends_with('_') {
        output.pop();
    }

    output
}

fn titleize(value: &str) -> String {
    value
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    let mut titleized = String::new();
                    titleized.push(first.to_ascii_uppercase());
                    titleized.push_str(&chars.as_str().to_ascii_lowercase());
                    titleized
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn percent_encode_path_segment(value: &str) -> String {
    let mut output = String::new();

    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                output.push(byte as char)
            }
            _ => output.push_str(&format!("%{byte:02X}")),
        }
    }

    output
}

fn sanitize_id_part(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use maud::Render;
    use seekwel::connection::Connection;

    #[seekwel::model]
    struct Person {
        id: u64,
        name: String,
        age: Option<u8>,
    }

    fn setup() -> Person {
        let _ = Connection::memory();
        let _ = Person::create_table();
        Person::builder()
            .name("Pat")
            .age(Some(42))
            .create()
            .unwrap()
    }

    fn draft() -> Person<seekwel::NewRecord> {
        Person::builder().name("Pat").age(Some(42)).build().unwrap()
    }

    #[test]
    fn renders_default_form_for_new_record() {
        let person = draft();
        let rendered = form_for(&person).render().into_string();

        assert!(
            rendered.contains(r#"<form action="/people" method="post">"#),
            "{rendered}"
        );
        assert!(!rendered.contains(r#"name="_method""#));
        assert!(rendered.contains(r#"<label for="person_name">Name</label>"#));
        assert!(
            rendered.contains(
                r#"<input type="text" name="person[name]" id="person_name" value="Pat">"#
            )
        );
        assert!(
            rendered
                .contains(r#"<input type="number" name="person[age]" id="person_age" value="42">"#)
        );
        assert!(rendered.contains(r#"<input type="submit" value="Save Person">"#));
    }

    #[test]
    fn renders_default_form_for_persisted_record() {
        let person = setup();
        let rendered = form_for(&person).render().into_string();

        assert!(
            rendered.contains(&format!(
                r#"<form action="/people/{}" method="post">"#,
                person.id
            )),
            "{rendered}"
        );
        assert!(rendered.contains(r#"<input type="hidden" name="_method" value="patch">"#));
    }

    #[test]
    fn renders_custom_fields() {
        let person = setup();
        let rendered = form_for(&person)
            .action("/profiles")
            .fields(|f| {
                html! {
                    (f.label(PersonColumns::Name, "Display name"))
                    (f.text_field(PersonColumns::Name))
                    (f.submit("Update"))
                }
            })
            .into_string();

        assert!(rendered.contains(r#"<form action="/profiles" method="post">"#));
        assert!(rendered.contains(r#"<label for="person_name">Display name</label>"#));
        assert!(
            rendered.contains(
                r#"<input type="text" name="person[name]" id="person_name" value="Pat">"#
            )
        );
        assert!(rendered.contains(r#"<input type="submit" value="Update">"#));
    }
}
