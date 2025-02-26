pub(crate) mod generate_hostname;
pub(crate) mod generate_net_config;
pub(crate) mod install;
pub(crate) mod node_ip;
pub(crate) mod prepare_primary_interface;
pub(crate) mod remove;
pub(crate) mod set_hostname;

pub(crate) use generate_hostname::GenerateHostnameArgs;
pub(crate) use generate_net_config::GenerateNetConfigArgs;
pub(crate) use install::InstallArgs;
pub(crate) use node_ip::NodeIpArgs;
pub(crate) use prepare_primary_interface::PreparePrimaryInterfaceArgs;
pub(crate) use remove::RemoveArgs;
use serde::{Deserialize, Serialize};
pub(crate) use set_hostname::SetHostnameArgs;
use snafu::ResultExt;

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum InterfaceType {
    Dhcp,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum InterfaceFamily {
    Ipv4,
    Ipv6,
}

// Implement `from_str()` so argh can attempt to deserialize args into their proper types
derive_fromstr_from_deserialize!(InterfaceType);
derive_fromstr_from_deserialize!(InterfaceFamily);

/// Helper function that serializes the input to JSON and prints it
fn print_json<S>(val: S) -> Result<()>
where
    S: AsRef<str> + Serialize,
{
    let val = val.as_ref();
    let output = serde_json::to_string(val).context(error::JsonSerializeSnafu { output: val })?;
    println!("{}", output);
    Ok(())
}

/// Potential errors during netdog execution
mod error {
    use crate::{lease, net_config, wicked};
    use snafu::Snafu;
    use std::io;
    use std::path::PathBuf;

    #[derive(Debug, Snafu)]
    #[snafu(visibility(pub(crate)))]
    #[allow(clippy::enum_variant_names)]
    pub(crate) enum Error {
        #[snafu(display("Failed to write current IP to '{}': {}", path.display(), source))]
        CurrentIpWriteFailed { path: PathBuf, source: io::Error },

        #[snafu(display("Failed to read current IP data in '{}': {}", path.display(), source))]
        CurrentIpReadFailed { path: PathBuf, source: io::Error },

        #[snafu(display("'systemd-sysctl' failed: {}", stderr))]
        FailedSystemdSysctl { stderr: String },

        #[snafu(display("Failed to discern primary interface"))]
        GetPrimaryInterface,

        #[snafu(display("Failed to write hostname to '{}': {}", path.display(), source))]
        HostnameWriteFailed { path: PathBuf, source: io::Error },

        #[snafu(display("Failed to write network interface configuration: {}", source))]
        InterfaceConfigWrite { source: wicked::Error },

        #[snafu(display("Invalid IP address '{}': {}", ip, source))]
        IpFromString {
            ip: String,
            source: std::net::AddrParseError,
        },

        #[snafu(display("Error serializing to JSON: '{}': {}", output, source))]
        JsonSerialize {
            output: String,
            source: serde_json::error::Error,
        },

        #[snafu(display("Failed to read/parse lease data in '{}': {}", path.display(), source))]
        LeaseParseFailed { path: PathBuf, source: lease::Error },

        #[snafu(display("Unable to read/parse network config from '{}': {}", path.display(), source))]
        NetConfigParse {
            path: PathBuf,
            source: net_config::Error,
        },

        #[snafu(display("Failed to write primary interface to '{}': {}", path.display(), source))]
        PrimaryInterfaceWrite { path: PathBuf, source: io::Error },

        #[snafu(display("Failed to read primary interface from '{}': {}", path.display(), source))]
        PrimaryInterfaceRead { path: PathBuf, source: io::Error },

        #[snafu(display("Failed to build resolver configuration: {}", source))]
        ResolvConfBuildFailed { source: std::fmt::Error },

        #[snafu(display("Failed to write resolver configuration to '{}': {}", path.display(), source))]
        ResolvConfWriteFailed { path: PathBuf, source: io::Error },

        #[snafu(display("Failed to build sysctl config: {}", source))]
        SysctlConfBuild { source: std::fmt::Error },

        #[snafu(display("Failed to write sysctl config to '{}': {}", path.display(), source))]
        SysctlConfWrite { path: PathBuf, source: io::Error },

        #[snafu(display("Failed to run 'systemd-sysctl': {}", source))]
        SystemdSysctlExecution { source: io::Error },
    }
}

pub(crate) type Result<T> = std::result::Result<T, error::Error>;
