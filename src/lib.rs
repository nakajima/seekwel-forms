use std::borrow::Cow;

use inflection::{plural, singular};
use maud::{Markup, Render, html};
use rusqlite::types::Value;
use seekwel::model::{Column, ColumnDef, Errors, Model, ModelRecord};

pub use maud;

pub fn form_for<M>(model: &M) -> FormFor<'_, M>
where
    M: ModelRecord,
{
    FormFor::new(model)
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
    pub fn form_method(self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::Post | Self::Put | Self::Patch | Self::Delete => "post",
        }
    }

    pub fn method_override(self) -> Option<&'static str> {
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

#[derive(Debug, Clone, Default)]
struct ElementAttrs {
    attrs: Vec<ElementAttr>,
}

impl ElementAttrs {
    fn set(&mut self, name: impl Into<String>, value: impl Render) {
        self.set_markup(name, value.render());
    }

    fn set_markup(&mut self, name: impl Into<String>, value: Markup) {
        let name = name.into();
        assert_valid_attribute_name(&name);

        if let Some(attr) = self.find_mut(&name) {
            attr.value = Some(value);
        } else {
            self.attrs.push(ElementAttr {
                name,
                value: Some(value),
            });
        }
    }

    fn set_empty(&mut self, name: impl Into<String>) {
        let name = name.into();
        assert_valid_attribute_name(&name);

        if let Some(attr) = self.find_mut(&name) {
            attr.value = None;
        } else {
            self.attrs.push(ElementAttr { name, value: None });
        }
    }

    fn id(&mut self, id: impl Into<String>) {
        self.set("id", id.into());
    }

    fn class(&mut self, class: impl Into<String>) {
        let value = class.into().render();

        if let Some(attr) = self.find_mut("class") {
            match &mut attr.value {
                Some(existing) => {
                    if !existing.0.is_empty() {
                        existing.0.push(' ');
                    }
                    existing.0.push_str(&value.0);
                }
                None => attr.value = Some(value),
            }
        } else {
            self.attrs.push(ElementAttr {
                name: "class".to_string(),
                value: Some(value),
            });
        }
    }

    fn extend(&mut self, attrs: &Self) {
        for attr in &attrs.attrs {
            match &attr.value {
                Some(value) => self.set_markup(attr.name.clone(), value.clone()),
                None => self.set_empty(attr.name.clone()),
            }
        }
    }

    fn render_to(&self, buffer: &mut String) {
        for attr in &self.attrs {
            attr.render_to(buffer);
        }
    }

    fn find_mut(&mut self, name: &str) -> Option<&mut ElementAttr> {
        self.attrs
            .iter_mut()
            .find(|attr| attr.name.eq_ignore_ascii_case(name))
    }
}

#[derive(Debug, Clone)]
struct ElementAttr {
    name: String,
    value: Option<Markup>,
}

impl ElementAttr {
    fn render_to(&self, buffer: &mut String) {
        buffer.push(' ');
        buffer.push_str(&self.name);

        if let Some(value) = &self.value {
            buffer.push_str("=\"");
            value.render_to(buffer);
            buffer.push('"');
        }
    }
}

#[derive(Debug, Clone)]
pub struct FormElement {
    tag: &'static str,
    attrs: ElementAttrs,
    body: Option<Markup>,
}

impl FormElement {
    fn new(tag: &'static str) -> Self {
        Self {
            tag,
            attrs: ElementAttrs::default(),
            body: None,
        }
    }

    fn with_body(tag: &'static str, body: Markup) -> Self {
        Self {
            tag,
            attrs: ElementAttrs::default(),
            body: Some(body),
        }
    }

    fn with_attrs_and_body(tag: &'static str, attrs: ElementAttrs, body: Markup) -> Self {
        Self {
            tag,
            attrs,
            body: Some(body),
        }
    }

    fn empty_attr(mut self, name: impl Into<String>) -> Self {
        self.attrs.set_empty(name);
        self
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.attrs.id(id);
        self
    }

    pub fn class(mut self, class: impl Into<String>) -> Self {
        self.attrs.class(class);
        self
    }

    pub fn attr(mut self, name: impl Into<String>, value: impl Render) -> Self {
        self.attrs.set(name, value);
        self
    }
}

impl Render for FormElement {
    fn render_to(&self, buffer: &mut String) {
        buffer.push('<');
        buffer.push_str(self.tag);
        self.attrs.render_to(buffer);
        buffer.push('>');

        if let Some(body) = &self.body {
            body.render_to(buffer);
            buffer.push_str("</");
            buffer.push_str(self.tag);
            buffer.push('>');
        }
    }
}

#[derive(Debug, Clone)]
pub struct CheckboxField {
    hidden: FormElement,
    checkbox: FormElement,
}

impl CheckboxField {
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.checkbox = self.checkbox.id(id);
        self
    }

    pub fn class(mut self, class: impl Into<String>) -> Self {
        self.checkbox = self.checkbox.class(class);
        self
    }

    pub fn attr(mut self, name: impl Into<String>, value: impl Render) -> Self {
        self.checkbox = self.checkbox.attr(name, value);
        self
    }
}

impl Render for CheckboxField {
    fn render_to(&self, buffer: &mut String) {
        self.hidden.render_to(buffer);
        self.checkbox.render_to(buffer);
    }
}

#[derive(Debug, Clone)]
pub struct FieldErrors<'a> {
    messages: Vec<&'a str>,
    attrs: ElementAttrs,
}

impl<'a> FieldErrors<'a> {
    fn new(messages: Vec<&'a str>) -> Self {
        let mut attrs = ElementAttrs::default();
        attrs.class("field-errors");
        Self { messages, attrs }
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.attrs.id(id);
        self
    }

    pub fn class(mut self, class: impl Into<String>) -> Self {
        self.attrs.class(class);
        self
    }

    pub fn attr(mut self, name: impl Into<String>, value: impl Render) -> Self {
        self.attrs.set(name, value);
        self
    }
}

impl Render for FieldErrors<'_> {
    fn render_to(&self, buffer: &mut String) {
        if self.messages.is_empty() {
            return;
        }

        buffer.push_str("<div");
        self.attrs.render_to(buffer);
        buffer.push('>');

        for message in &self.messages {
            buffer.push_str(r#"<span class="field-error">"#);
            message.render_to(buffer);
            buffer.push_str("</span>");
        }

        buffer.push_str("</div>");
    }
}

#[derive(Debug, Clone)]
pub struct FormFor<'a, M>
where
    M: ModelRecord,
{
    model: &'a M,
    action: Option<String>,
    method: Option<FormMethod>,
    attrs: ElementAttrs,
    param_name: Option<String>,
    errors: &'a Errors<M::Column>,
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
            attrs: ElementAttrs::default(),
            param_name: None,
            errors: model.errors(),
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
        self.attrs.id(id);
        self
    }

    pub fn class(mut self, class: impl Into<String>) -> Self {
        self.attrs.class(class);
        self
    }

    pub fn attr(mut self, name: impl Into<String>, value: impl Render) -> Self {
        self.attrs.set(name, value);
        self
    }

    pub fn param_name(mut self, param_name: impl Into<String>) -> Self {
        self.param_name = Some(param_name.into());
        self
    }

    pub fn errors(mut self, errors: &'a Errors<M::Column>) -> Self {
        self.errors = errors;
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

    pub fn fields<F, R>(self, fields: F) -> Markup
    where
        F: FnOnce(&FormBuilder<'_, M>) -> R,
        R: Render,
    {
        let builder = self.builder();
        let body = fields(&builder).render();
        self.render_form(body)
    }

    pub fn builder(&self) -> FormBuilder<'_, M> {
        FormBuilder::new(self.model, self.resolved_param_name(), self.errors)
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

        let body = html! {
            @if let Some(override_method) = override_method {
                input type="hidden" name="_method" value=(override_method);
            }
            (body)
        };
        let mut attrs = ElementAttrs::default();
        attrs.set("action", action);
        attrs.set("method", form_method);
        attrs.extend(&self.attrs);

        FormElement::with_attrs_and_body("form", attrs, body).render()
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
    errors: &'a Errors<M::Column>,
    fields: Vec<FormField>,
}

impl<'a, M> FormBuilder<'a, M>
where
    M: Model,
{
    fn new(model: &'a M, param_name: String, errors: &'a Errors<M::Column>) -> Self {
        Self {
            model,
            param_name,
            errors,
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

    pub fn errors(&self) -> &Errors<M::Column> {
        self.errors
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

    pub fn error_messages<C>(&self, column: C) -> Vec<&str>
    where
        C: Column,
    {
        self.error_messages_named(column.as_str())
    }

    pub fn error_messages_named(&self, column_name: impl AsRef<str>) -> Vec<&str> {
        let column_name = column_name.as_ref();

        self.errors
            .all()
            .iter()
            .filter_map(|error| {
                error
                    .column()
                    .is_some_and(|column| column.as_str() == column_name)
                    .then_some(error.message())
            })
            .collect()
    }

    pub fn field_errors<C>(&self, column: C) -> FieldErrors<'_>
    where
        C: Column,
    {
        self.field_errors_named(column.as_str())
    }

    pub fn field_errors_named(&self, column_name: impl AsRef<str>) -> FieldErrors<'_> {
        FieldErrors::new(self.error_messages_named(column_name))
    }

    pub fn label<C>(&self, column: C, text: impl Render) -> FormElement
    where
        C: Column,
    {
        self.label_named(column.as_str(), text)
    }

    pub fn label_named(&self, column_name: impl AsRef<str>, text: impl Render) -> FormElement {
        let id = self.field_id_named(column_name);
        FormElement::with_body("label", text.render()).attr("for", id)
    }

    pub fn text_field<C>(&self, column: C) -> FormElement
    where
        C: Column,
    {
        self.input("text", column)
    }

    pub fn text_field_named(&self, column_name: impl AsRef<str>) -> FormElement {
        self.input_named("text", column_name)
    }

    pub fn number_field<C>(&self, column: C) -> FormElement
    where
        C: Column,
    {
        self.input("number", column)
    }

    pub fn number_field_named(&self, column_name: impl AsRef<str>) -> FormElement {
        self.input_named("number", column_name)
    }

    pub fn hidden_field<C>(&self, column: C) -> FormElement
    where
        C: Column,
    {
        self.input("hidden", column)
    }

    pub fn hidden_field_named(&self, column_name: impl AsRef<str>) -> FormElement {
        self.input_named("hidden", column_name)
    }

    pub fn textarea<C>(&self, column: C) -> FormElement
    where
        C: Column,
    {
        self.textarea_named(column.as_str())
    }

    pub fn textarea_named(&self, column_name: impl AsRef<str>) -> FormElement {
        let column_name = column_name.as_ref();
        let name = self.field_name_named(column_name);
        let id = self.field_id_named(column_name);
        let value = self
            .value_named(column_name)
            .and_then(FieldValue::as_input_value)
            .unwrap_or_default();

        FormElement::with_body("textarea", value.render())
            .attr("name", name)
            .id(id)
    }

    pub fn checkbox<C>(&self, column: C) -> CheckboxField
    where
        C: Column,
    {
        self.checkbox_named(column.as_str())
    }

    pub fn checkbox_named(&self, column_name: impl AsRef<str>) -> CheckboxField {
        let column_name = column_name.as_ref();
        let name = self.field_name_named(column_name);
        let id = self.field_id_named(column_name);
        let checked = self
            .value_named(column_name)
            .is_some_and(FieldValue::is_truthy);

        let hidden = FormElement::new("input")
            .attr("type", "hidden")
            .attr("name", name.clone())
            .attr("value", "0");
        let mut checkbox = FormElement::new("input")
            .attr("type", "checkbox")
            .attr("name", name)
            .id(id)
            .attr("value", "1");

        if checked {
            checkbox = checkbox.empty_attr("checked");
        }

        CheckboxField { hidden, checkbox }
    }

    pub fn submit(&self, label: impl Render) -> FormElement {
        FormElement::new("input")
            .attr("type", "submit")
            .attr("value", label)
    }

    pub fn input<C>(&self, input_type: impl AsRef<str>, column: C) -> FormElement
    where
        C: Column,
    {
        self.input_named(input_type, column.as_str())
    }

    pub fn input_named(
        &self,
        input_type: impl AsRef<str>,
        column_name: impl AsRef<str>,
    ) -> FormElement {
        let column_name = column_name.as_ref();
        let name = self.field_name_named(column_name);
        let id = self.field_id_named(column_name);
        let value = self
            .value_named(column_name)
            .and_then(FieldValue::as_input_value);
        let mut input = FormElement::new("input")
            .attr("type", input_type.as_ref())
            .attr("name", name)
            .id(id);

        if let Some(value) = value {
            input = input.attr("value", value);
        }

        input
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
                    (self.field_errors_named(field.name))
                }
            }
            @if include_submit {
                (self.submit(submit_label.as_ref()))
            }
        }
    }

    fn default_input(&self, field: &FormField) -> Markup {
        match field.sql_type {
            "INTEGER" => self.number_field_named(field.name).render(),
            "REAL" => {
                let column_name = field.name;
                let name = self.field_name_named(column_name);
                let id = self.field_id_named(column_name);
                let mut input = FormElement::new("input")
                    .attr("type", "number")
                    .attr("step", "any")
                    .attr("name", name)
                    .id(id);

                if let Some(value) = field.value.as_input_value() {
                    input = input.attr("value", value);
                }

                input.render()
            }
            "BLOB" => FormElement::new("input")
                .attr("type", "file")
                .attr("name", self.field_name_named(field.name))
                .id(self.field_id_named(field.name))
                .render(),
            _ => self.text_field_named(field.name).render(),
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

fn assert_valid_attribute_name(name: &str) {
    assert!(
        is_valid_attribute_name(name),
        "invalid HTML attribute name: {name:?}"
    );
}

fn is_valid_attribute_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|ch| {
            !ch.is_ascii_whitespace() && !matches!(ch, '"' | '\'' | '<' | '>' | '/' | '=')
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use maud::Render;
    use seekwel::NewModel;
    use seekwel::connection::Connection;

    struct PersonValidator;

    #[seekwel::model(validator = PersonValidator)]
    struct Person {
        id: u64,
        name: String,
        age: Option<u8>,
    }

    impl<S> seekwel::Validator<Person<S>> for PersonValidator {
        fn validate(person: &Person<S>, errors: &mut seekwel::Errors<PersonColumns>) {
            if person.name.trim().is_empty() {
                errors.add(PersonColumns::Name, "is required");
            }
        }
    }

    fn setup() -> Person {
        let _ = Connection::memory();
        let _ = <Person as seekwel::Model>::create_table();
        Person::builder()
            .name("Pat")
            .age(Some(42))
            .create()
            .unwrap()
    }

    fn draft() -> Person<seekwel::NewRecord> {
        Person::builder().name("Pat").age(Some(42)).build().unwrap()
    }

    fn invalid_draft() -> Person<seekwel::Invalid<seekwel::NewRecord, PersonColumns>> {
        let person = Person::builder().name("").age(Some(42)).build().unwrap();

        match person.save() {
            Err(seekwel::SaveError::Invalid(invalid)) => invalid,
            _ => panic!("expected invalid model"),
        }
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
    fn renders_field_errors_for_invalid_record() {
        let person = invalid_draft();
        let rendered = form_for(&person).render().into_string();

        assert!(!rendered.contains(r#"class="errors""#));
        assert!(
            rendered.contains(
                r#"<div class="field-errors"><span class="field-error">is required</span></div>"#
            ),
            "{rendered}"
        );
    }

    #[test]
    fn renders_custom_field_errors() {
        let person = invalid_draft();
        let rendered = form_for(&person)
            .fields(|f| {
                html! {
                    (f.field_errors(PersonColumns::Name))
                }
            })
            .into_string();

        assert_eq!(
            rendered,
            r#"<form action="/people" method="post"><div class="field-errors"><span class="field-error">is required</span></div></form>"#
        );
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

    #[test]
    fn renders_custom_form_attributes() {
        let person = draft();
        let rendered = form_for(&person)
            .id("person-form")
            .class("stack")
            .class("wide")
            .attr("data-controller", "people")
            .without_submit()
            .render()
            .into_string();

        assert!(
            rendered.contains(
                r#"<form action="/people" method="post" id="person-form" class="stack wide" data-controller="people">"#
            ),
            "{rendered}"
        );
    }

    #[test]
    fn renders_custom_element_attributes() {
        let person = setup();
        let rendered = form_for(&person)
            .fields(|f| {
                html! {
                    (f.label(PersonColumns::Name, "Display name")
                        .class("label")
                        .attr("data-label", "name"))
                    (f.text_field(PersonColumns::Name)
                        .id("display_name")
                        .class("input")
                        .attr("data-controller", "person-name")
                        .attr("data-value", "\"Pat\" & <Pat>"))
                    (f.textarea(PersonColumns::Name).class("textarea"))
                    (f.submit("Update").class("button"))
                }
            })
            .into_string();

        assert!(
            rendered.contains(
                r#"<label for="person_name" class="label" data-label="name">Display name</label>"#
            ),
            "{rendered}"
        );
        assert!(
            rendered.contains(
                r#"<input type="text" name="person[name]" id="display_name" value="Pat" class="input" data-controller="person-name" data-value="&quot;Pat&quot; &amp; &lt;Pat&gt;">"#
            ),
            "{rendered}"
        );
        assert!(
            rendered.contains(
                r#"<textarea name="person[name]" id="person_name" class="textarea">Pat</textarea>"#
            ),
            "{rendered}"
        );
        assert!(
            rendered.contains(r#"<input type="submit" value="Update" class="button">"#),
            "{rendered}"
        );
    }

    #[test]
    fn renders_custom_field_error_attributes() {
        let person = invalid_draft();
        let rendered = form_for(&person)
            .fields(|f| {
                html! {
                    (f.field_errors(PersonColumns::Name)
                        .id("name-errors")
                        .class("stack")
                        .attr("data-errors", "name"))
                }
            })
            .into_string();

        assert_eq!(
            rendered,
            r#"<form action="/people" method="post"><div class="field-errors stack" id="name-errors" data-errors="name"><span class="field-error">is required</span></div></form>"#
        );
    }

    #[test]
    fn renders_custom_checkbox_attributes_on_visible_input() {
        let person = draft();
        let rendered = form_for(&person)
            .fields(|f| {
                html! {
                    (f.checkbox(PersonColumns::Age)
                        .id("age_check")
                        .class("check")
                        .attr("data-checkbox", "age"))
                }
            })
            .into_string();

        assert_eq!(
            rendered,
            r#"<form action="/people" method="post"><input type="hidden" name="person[age]" value="0"><input type="checkbox" name="person[age]" id="age_check" value="1" checked class="check" data-checkbox="age"></form>"#
        );
    }
}
