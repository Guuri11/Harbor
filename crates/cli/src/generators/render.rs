use include_dir::{Dir, include_dir};
use tera::Tera;

static TEMPLATES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/templates");

pub fn render(template_name: &str, ctx: &tera::Context) -> Result<String, tera::Error> {
    let content = TEMPLATES
        .get_file(template_name)
        .ok_or_else(|| tera::Error::msg(format!("template not found: {template_name}")))?
        .contents_utf8()
        .ok_or_else(|| tera::Error::msg("template not UTF-8"))?;
    let mut tera = Tera::default();
    tera.add_raw_template(template_name, content)?;
    tera.render(template_name, ctx)
}
