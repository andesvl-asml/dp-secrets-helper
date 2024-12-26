use anyhow::Result;
use k8s_openapi::serde::Deserialize;
use kube::api::DynamicObject;
use serde_yml::Deserializer;
use std::{path::PathBuf, rc::Rc};

#[derive(Debug, Clone)]
struct SystemManifests {
    directory: PathBuf,
    platforms: Vec<Platform>,
}

#[derive(Debug, Clone)]
struct Platform {
    name: String,
    environment_directory: PathBuf,
    cluster_directory: PathBuf,
    manifests_directory: PathBuf,
    components: Vec<Rc<Component>>,
}

#[derive(Debug, Clone)]
struct Component {
    manifests_directory: PathBuf,
    name: String,
}

#[derive(Clone)]
struct ManifestResource {
    file: PathBuf,
    component: Rc<Component>,
    platform: Rc<Platform>,
    resource: DynamicObject,
}

struct PlatformResourceIterator<'a> {
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
