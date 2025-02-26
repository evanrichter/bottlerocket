//! The template module owns the finding and rendering of parameter templates that used to generate
//! SSM parameter names and values.

use super::{BuildContext, SsmKey, SsmParameters};
use crate::aws::ami::Image;
use log::trace;
use rusoto_core::Region;
use serde::{Deserialize, Serialize};
use snafu::{ensure, ResultExt};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tinytemplate::TinyTemplate;

/// Represents a single SSM parameter
#[derive(Debug, Deserialize)]
pub(crate) struct TemplateParameter {
    pub(crate) name: String,
    pub(crate) value: String,

    // User can say parameters only apply to these variants/arches
    #[serde(default, rename = "variant")]
    pub(crate) variants: Vec<String>,
    #[serde(default, rename = "arch")]
    pub(crate) arches: Vec<String>,
}

/// Represents a set of SSM parameters, in a format that allows for clear definition of
/// parameters in TOML files
#[derive(Debug, Deserialize)]
pub(crate) struct TemplateParameters {
    // In a TOML table, it's clearer to define a single entry as a "parameter".
    #[serde(default, rename = "parameter")]
    pub(crate) parameters: Vec<TemplateParameter>,
}

/// Deserializes template parameters from the template file, taking into account conditional
/// parameters that may or may not apply based on our build context.
pub(crate) fn get_parameters(
    template_path: &Path,
    build_context: &BuildContext<'_>,
) -> Result<TemplateParameters> {
    let templates_str = fs::read_to_string(&template_path).context(error::FileSnafu {
        op: "read",
        path: &template_path,
    })?;
    let mut template_parameters: TemplateParameters =
        toml::from_str(&templates_str).context(error::InvalidTomlSnafu {
            path: &template_path,
        })?;
    trace!("Parsed templates: {:#?}", template_parameters);

    // You shouldn't point to an empty file, but if all the entries are removed by
    // conditionals below, we allow that and just don't set any parameters.
    ensure!(
        !template_parameters.parameters.is_empty(),
        error::NoTemplatesSnafu {
            path: template_path
        }
    );

    let variant = build_context.variant.to_string();
    let arch = build_context.arch.to_string();
    template_parameters.parameters.retain(|p| {
        (p.variants.is_empty() || p.variants.contains(&variant))
            && (p.arches.is_empty() || p.arches.contains(&arch))
    });
    trace!("Templates after conditionals: {:#?}", template_parameters);

    Ok(template_parameters)
}

/// Render the given template parameters using the data from the given AMIs
pub(crate) fn render_parameters(
    template_parameters: TemplateParameters,
    amis: HashMap<Region, Image>,
    ssm_prefix: &str,
    build_context: &BuildContext<'_>,
) -> Result<SsmParameters> {
    /// Values that we allow as template variables
    #[derive(Debug, Serialize)]
    struct TemplateContext<'a> {
        variant: &'a str,
        arch: &'a str,
        image_id: &'a str,
        image_name: &'a str,
        image_version: &'a str,
        region: &'a str,
    }
    let mut new_parameters = HashMap::new();
    for (region, image) in amis {
        let context = TemplateContext {
            variant: build_context.variant,
            arch: build_context.arch,
            image_id: &image.id,
            image_name: &image.name,
            image_version: build_context.image_version,
            region: region.name(),
        };

        for tp in &template_parameters.parameters {
            let mut tt = TinyTemplate::new();
            tt.add_template("name", &tp.name)
                .context(error::AddTemplateSnafu { template: &tp.name })?;
            tt.add_template("value", &tp.value)
                .context(error::AddTemplateSnafu {
                    template: &tp.value,
                })?;
            let name_suffix = tt
                .render("name", &context)
                .context(error::RenderTemplateSnafu { template: &tp.name })?;
            let value = tt
                .render("value", &context)
                .context(error::RenderTemplateSnafu {
                    template: &tp.value,
                })?;

            new_parameters.insert(
                SsmKey::new(region.clone(), join_name(ssm_prefix, &name_suffix)),
                value,
            );
        }
    }

    Ok(new_parameters)
}

/// Render the names of the given template parameters using the fixed data about the current build.
/// Returns a mapping of templated name to rendered name, so we can associate rendered names to a
/// common source name
pub(crate) fn render_parameter_names(
    template_parameters: &TemplateParameters,
    ssm_prefix: &str,
    build_context: &BuildContext<'_>,
) -> Result<HashMap<String, String>> {
    let mut new_parameters = HashMap::new();
    for tp in &template_parameters.parameters {
        let mut tt = TinyTemplate::new();
        tt.add_template("name", &tp.name)
            .context(error::AddTemplateSnafu { template: &tp.name })?;
        let name_suffix = tt
            .render("name", &build_context)
            .context(error::RenderTemplateSnafu { template: &tp.name })?;
        new_parameters.insert(tp.name.clone(), join_name(ssm_prefix, &name_suffix));
    }

    Ok(new_parameters)
}

/// Make sure prefix and parameter name are separated by one slash
fn join_name(ssm_prefix: &str, name_suffix: &str) -> String {
    if ssm_prefix.ends_with('/') && name_suffix.starts_with('/') {
        format!("{}{}", ssm_prefix, &name_suffix[1..])
    } else if ssm_prefix.ends_with('/') || name_suffix.starts_with('/') {
        format!("{}{}", ssm_prefix, name_suffix)
    } else {
        format!("{}/{}", ssm_prefix, name_suffix)
    }
}

mod error {
    use snafu::Snafu;
    use std::io;
    use std::path::PathBuf;

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub(super)))]
    pub(crate) enum Error {
        #[snafu(display("Error building template from '{}': {}", template, source))]
        AddTemplate {
            template: String,
            source: tinytemplate::error::Error,
        },

        #[snafu(display("Failed to {} '{}': {}", op, path.display(), source))]
        File {
            op: String,
            path: PathBuf,
            source: io::Error,
        },

        #[snafu(display("Invalid config file at '{}': {}", path.display(), source))]
        InvalidToml {
            path: PathBuf,
            source: toml::de::Error,
        },

        #[snafu(display("Found no parameter templates in {}", path.display()))]
        NoTemplates { path: PathBuf },

        #[snafu(display("Error rendering template from '{}': {}", template, source))]
        RenderTemplate {
            template: String,
            source: tinytemplate::error::Error,
        },
    }
}
pub(crate) use error::Error;
type Result<T> = std::result::Result<T, error::Error>;
