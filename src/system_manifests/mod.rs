use anyhow::{Context, Result};
use k8s_openapi::serde::{Deserialize, Serialize};
use kube::api::{DynamicObject, ObjectMeta};
use serde_yaml::Deserializer;
use std::{path::PathBuf, rc::Rc};

use crate::Cli;

#[derive(Debug, Clone)]
pub struct SystemManifests {
    pub directory: PathBuf,
    pub platforms: Vec<Rc<Platform>>,
}

fn validate_directories_exist(directories: &[&PathBuf]) -> Result<()> {
    for dir in directories {
        if let Ok(metadata) = std::fs::metadata(dir) {
            if !metadata.is_dir() {
                anyhow::bail!("Path exists but is not a directory: {}", dir.display());
            }
        } else {
            anyhow::bail!("Directory does not exist: {}", dir.display());
        }
    }
    Ok(())
}

fn get_cluster_names_from_clusters_directories(
    clusters_directory: &PathBuf,
) -> Result<Vec<String>> {
    let mut platforms = Vec::new();

    let entries = std::fs::read_dir(clusters_directory).with_context(|| {
        format!(
            "Failed to read clusters directory to discover platforms: {}",
            clusters_directory.display()
        )
    })?;

    for entry in entries {
        let entry = entry.with_context(|| "Failed to read a platform directory")?;
        let path = entry.path();

        if path.is_dir() {
            platforms.push(
                path.file_stem()
                    .map(|os_string| os_string.to_str())
                    .flatten()
                    .with_context(|| "Failed to read a platform directory name")?
                    .to_owned(),
            );
        }
    }

    Ok(platforms)
}

impl SystemManifests {
    pub fn new(cli: &Cli) -> Result<Self> {
        let directory: PathBuf = cli.system_manifests.clone().into();
        let clusters_directory = directory.join("clusters");
        validate_directories_exist(&[&clusters_directory])
            .with_context(|| "Failed to obtain clusters directory")?;
        let platforms = get_cluster_names_from_clusters_directories(&clusters_directory)?
            .into_iter()
            .map(|name| Platform::new(name, directory.clone()).map(Rc::new))
            .collect::<Result<_>>()?;
        Ok(SystemManifests {
            directory,
            platforms,
        })
    }
}

impl<'a> SystemManifests {
    pub fn resource_iter(&'a self) -> SystemManifestsResourceIterator<'a> {
        SystemManifestsResourceIterator::new(self)
    }
}

pub struct SystemManifestsResourceIterator<'a> {
    resource_iterator: Box<dyn Iterator<Item = anyhow::Result<ManifestResource>> + 'a>,
}

impl<'a> SystemManifestsResourceIterator<'a> {
    fn new(system_manifests: &'a SystemManifests) -> Self {
        let resource_iterator = system_manifests
            .platforms
            .iter()
            .flat_map(|p| p.resource_iter());

        SystemManifestsResourceIterator {
            resource_iterator: Box::new(resource_iterator),
        }
    }
}

impl<'a> Iterator for SystemManifestsResourceIterator<'a> {
    type Item = Result<ManifestResource>;

    fn next(&mut self) -> Option<Self::Item> {
        self.resource_iterator.next()
    }
}

#[derive(Debug, Clone)]
pub struct Platform {
    pub name: String,
    pub environment_directory: PathBuf,
    pub cluster_directory: PathBuf,
    pub manifests_directory: PathBuf,
    pub components: Vec<Rc<Component>>,
}

fn get_component_names_from_manifest_directory(
    platform_manifests_directory: &PathBuf,
) -> Result<Vec<String>> {
    let mut components = Vec::new();

    let entries = std::fs::read_dir(platform_manifests_directory).with_context(|| {
        format!(
            "Failed to read platform manifests directory to discover components: {}",
            platform_manifests_directory.display()
        )
    })?;

    for entry in entries {
        let entry = entry.with_context(|| "Failed to read the manifest directory")?;
        let path = entry.path();

        if path.is_dir() {
            components.push(
                path.file_stem()
                    .map(|os_string| os_string.to_str())
                    .flatten()
                    .with_context(|| "Failed to read a component manifest directory")?
                    .to_owned(),
            );
        }
    }

    Ok(components)
}

impl Platform {
    pub fn new(name: String, system_manifest_directory: PathBuf) -> Result<Self> {
        let environment_directory = system_manifest_directory
            .join("environments")
            .join(name.clone());
        let cluster_directory = system_manifest_directory
            .join("clusters")
            .join(name.clone());
        let manifests_directory = system_manifest_directory
            .join("manifests")
            .join(name.clone());
        validate_directories_exist(&[
            &environment_directory,
            &cluster_directory,
            &manifests_directory,
        ])
        .with_context(|| "Failed to obtain platform directories")?;
        let components: Vec<Rc<Component>> =
            get_component_names_from_manifest_directory(&manifests_directory)?
                .into_iter()
                .map(|name| {
                    let component_manifests_directory = manifests_directory.join(name.clone());
                    validate_directories_exist(&[&component_manifests_directory])
                        .with_context(|| "Failed to obtain component manifest directory")?;
                    Ok(Rc::new(Component {
                        name,
                        manifests_directory: component_manifests_directory,
                    }))
                })
                .collect::<Result<_>>()?;
        Ok(Platform {
            name,
            environment_directory,
            cluster_directory,
            manifests_directory,
            components,
        })
    }

    fn resource_iter(self: &Rc<Self>) -> PlatformResourceIterator {
        PlatformResourceIterator::new(self)
    }
}

#[derive(Debug, Clone)]
pub struct Component {
    pub manifests_directory: PathBuf,
    pub name: String,
}

#[derive(Clone)]
pub struct ManifestResource {
    pub file: PathBuf,
    pub component: Rc<Component>,
    pub platform: Rc<Platform>,
    pub resource: DynamicObject,
}

#[derive(Debug, Clone, Serialize)]
pub struct FlatManifestResource {
    pub file: PathBuf,
    pub component_name: String,
    pub platform_name: String,
    pub resource_meta: kube::core::ObjectMeta,
}

impl From<ManifestResource> for FlatManifestResource {
    fn from(value: ManifestResource) -> Self {
        FlatManifestResource {
            file: value.file.clone(),
            component_name: value.component.name.clone(),
            platform_name: value.platform.name.clone(),
            resource_meta: value.resource.metadata,
        }
    }
}

pub struct PlatformResourceIterator<'a> {
    platform: Rc<Platform>,
    resource_iterator: Box<dyn Iterator<Item = anyhow::Result<ManifestResource>> + 'a>,
}

impl<'a> PlatformResourceIterator<'a> {
    fn new(platform: &'a Rc<Platform>) -> PlatformResourceIterator<'a> {
        let platform_clone: Rc<Platform> = platform.clone();

        let resource_iterator = platform
            .components
            .iter()
            .cloned()
            .to_owned()
            .flat_map(|c: Rc<Component>| {
                std::fs::read_dir(&c.manifests_directory)
                    .into_iter()
                    .flat_map(move |rd| {
                        rd.into_iter()
                            .filter(|dr| match dr {
                                Ok(dir_entry) => {
                                    dir_entry.path().is_file()
                                        && dir_entry
                                            .path()
                                            .extension()
                                            .map_or(false, |ext| ext == "yaml" || ext == "yml")
                                }
                                _ => true, // propagate errors
                            })
                            .map({
                                let c = c.clone();
                                move |dr| {
                                    let c = c.clone();
                                    dr.map(move |dir_entry: std::fs::DirEntry| (c, dir_entry))
                                }
                            })
                    })
            })
            .flat_map(move |file_res| {
                file_res
                    .into_iter()
                    .flat_map({
                        let platform_clone = platform_clone.clone();
                        move |(c, dir_entry)| {
                            let c = c.clone();
                            std::fs::File::open(dir_entry.path())
                                .map(|file| std::io::BufReader::new(file))
                                .map({
                                    let platform_clone = platform_clone.clone();
                                    move |reader: std::io::BufReader<std::fs::File>| {
                                        Deserializer::from_reader(reader).into_iter().map(
                                            move |doc| {
                                                DynamicObject::deserialize(doc)
                                                    .map_err(anyhow::Error::from)
                                                    .map({
                                                        let component = c.clone();
                                                        let platform = platform_clone.clone();
                                                        let file = dir_entry.path().to_owned();
                                                        move |resource| ManifestResource {
                                                            file,
                                                            component,
                                                            platform,
                                                            resource,
                                                        }
                                                    })
                                            },
                                        )
                                    }
                                })
                        }
                    })
                    .flatten()
            });

        PlatformResourceIterator {
            platform: platform.clone(),
            resource_iterator: Box::new(resource_iterator),
        }
    }
}

impl<'a> Iterator for PlatformResourceIterator<'a> {
    type Item = Result<ManifestResource>;

    fn next(&mut self) -> Option<Self::Item> {
        self.resource_iterator.next()
    }
}
