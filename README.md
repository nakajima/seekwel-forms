# seekwel-forms

Barebones HTML form helpers for [`seekwel`](https://crates.io/crates/seekwel) models, rendered with [`maud`](https://crates.io/crates/maud).

## What it does

`seekwel-forms` generates form markup from a Seekwel model record:

- Builds default form actions from the model table name and primary key.
- Uses `post` for new records and `patch` via `_method` for persisted records.
- Names fields like `person[name]` and ids like `person_name`.
- Renders labels, inputs, submit buttons, and field-level validation errors.
- Supports `id`, `class`, and custom attributes on forms and generated controls.
- Re-exports `maud` for custom form markup.

## Installation

```toml
[dependencies]
seekwel-forms = "0.1"
```

## Usage

Render the default form for a model:

```rust
use seekwel_forms::{form_for, maud::Render};

let html = form_for(&person).render().into_string();
```

Customize the action, method, labels, and fields:

```rust
use seekwel_forms::{
    form_for,
    maud::{html, Render},
};

let html = form_for(&person)
    .action("/people")
    .post()
    .id("person-form")
    .class("stack")
    .attr("data-controller", "person")
    .fields(|f| {
        html! {
            (f.label(PersonColumns::Name, "Name").class("label"))
            (f.text_field(PersonColumns::Name)
                .class("input")
                .attr("data-field", "name"))
            (f.field_errors(PersonColumns::Name).class("errors"))
            (f.submit("Save").class("button"))
        }
    })
    .into_string();
```

## Development

```sh
cargo test
```

## License

BSD-3-Clause
