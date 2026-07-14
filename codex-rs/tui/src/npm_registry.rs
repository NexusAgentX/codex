use serde::Deserialize;
use std::collections::HashMap;
use url::Url;

const NPM_REGISTRY_URL: &str = "https://registry.npmjs.org/";

#[derive(Deserialize, Debug, Clone)]
pub(crate) struct NpmPackageInfo {
    #[serde(rename = "dist-tags")]
    dist_tags: HashMap<String, String>,
    versions: HashMap<String, NpmPackageVersionInfo>,
}

#[derive(Deserialize, Debug, Clone)]
struct NpmPackageVersionInfo {
    dist: Option<NpmPackageDist>,
}

#[derive(Deserialize, Debug, Clone)]
struct NpmPackageDist {
    tarball: Option<String>,
    integrity: Option<String>,
}

pub(crate) fn package_url(package_name: &str) -> anyhow::Result<Url> {
    let mut url = Url::parse(NPM_REGISTRY_URL)?;
    url.path_segments_mut()
        .map_err(|_| anyhow::anyhow!("npm registry URL cannot contain package paths"))?
        .push(package_name);
    Ok(url)
}

pub(crate) fn latest_ready_version(package_info: &NpmPackageInfo) -> anyhow::Result<String> {
    let version = package_info
        .dist_tags
        .get("latest")
        .map(String::as_str)
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .ok_or_else(|| anyhow::anyhow!("npm package is missing latest dist-tag"))?;

    version_info_with_dist(package_info, version)?;
    Ok(version.to_string())
}

fn version_info_with_dist<'a>(
    package_info: &'a NpmPackageInfo,
    version: &str,
) -> anyhow::Result<&'a NpmPackageVersionInfo> {
    let info = package_info
        .versions
        .get(version)
        .ok_or_else(|| anyhow::anyhow!("npm package version {version} is missing"))?;
    let Some(dist) = info.dist.as_ref() else {
        anyhow::bail!("npm package version {version} is missing dist metadata");
    };
    let has_tarball = dist
        .tarball
        .as_deref()
        .is_some_and(|tarball| !tarball.is_empty());
    if !has_tarball {
        anyhow::bail!("npm package version {version} is missing dist.tarball");
    }
    let has_integrity = dist
        .integrity
        .as_ref()
        .is_some_and(|integrity| !integrity.is_empty());
    if !has_integrity {
        anyhow::bail!("npm package version {version} is missing dist.integrity");
    }
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version_json(version: &str) -> serde_json::Value {
        serde_json::json!({
            "dist": {
                "integrity": format!("sha512-{version}"),
                "tarball": format!("https://registry.npmjs.org/@openai/codex/-/codex-{version}.tgz"),
            }
        })
    }

    fn package_info(version: &str, npm_latest: &str) -> NpmPackageInfo {
        let mut versions = serde_json::Map::new();
        versions.insert(version.to_string(), version_json(version));

        serde_json::from_value(serde_json::json!({
            "dist-tags": { "latest": npm_latest },
            "versions": serde_json::Value::Object(versions),
        }))
        .expect("valid npm package metadata")
    }

    #[test]
    fn package_url_encodes_scoped_package_as_one_path_segment() {
        assert_eq!(
            package_url("@nexus-agent-x/codex")
                .expect("valid registry URL")
                .as_str(),
            "https://registry.npmjs.org/@nexus-agent-x%2Fcodex"
        );
    }

    #[test]
    fn ready_version_comes_from_latest_dist_tag_and_requires_root_dist() {
        let latest = "1.2.3-nexus.4";
        let package_info = package_info(latest, latest);

        assert_eq!(
            latest_ready_version(&package_info).expect("npm package is ready"),
            latest
        );
    }

    #[test]
    fn ready_version_rejects_latest_tag_without_version_metadata() {
        let package_info = package_info("1.2.3", "1.2.2");

        let err = latest_ready_version(&package_info)
            .expect_err("npm latest dist-tag must resolve to package metadata");
        assert!(
            err.to_string().contains("version 1.2.2 is missing"),
            "error should name the missing latest version: {err}"
        );
    }

    #[test]
    fn ready_version_rejects_missing_root_dist() {
        let package_info: NpmPackageInfo = serde_json::from_value(serde_json::json!({
            "dist-tags": { "latest": "1.2.3" },
            "versions": { "1.2.3": {} },
        }))
        .expect("valid npm package metadata");

        let err =
            latest_ready_version(&package_info).expect_err("root package needs dist metadata");
        assert!(
            err.to_string().contains("missing dist metadata"),
            "error should name missing dist metadata: {err}"
        );
    }
}
