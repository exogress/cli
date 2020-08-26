use git2::build::RepoBuilder;
use git2::{Cred, IndexAddOption, PushOptions, RemoteCallbacks, Repository, Signature};
use std::fs;
use std::path::Path;
use tempfile::{tempdir, TempDir};
use url::Url;

pub struct Repo {
    dir: TempDir,
    repo: Repository,
}

impl Repo {
    pub fn new(repo_url: Url) -> Result<Self, anyhow::Error> {
        let clone_to = tempdir()?;
        info!("clone to {:?}", clone_to);
        let repo = RepoBuilder::new().clone(repo_url.as_str(), clone_to.path())?;
        Ok(Repo {
            dir: clone_to,
            repo,
        })
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn add_file(
        &self,
        filename: impl AsRef<Path>,
        content: impl AsRef<[u8]>,
    ) -> Result<(), anyhow::Error> {
        let absolute_path = self.dir.path().join(filename.as_ref());
        fs::write(absolute_path.clone(), content)?;
        info!("{:?} saved", absolute_path.to_str());
        let mut index = self.repo.index()?;
        info!("add {:?}", filename.as_ref().to_str());
        index.add_all(
            [filename.as_ref().to_str().unwrap()].iter(),
            IndexAddOption::DEFAULT,
            None,
        )?;

        Ok(index.write()?)
    }

    pub fn commit(&self, msg: &str) -> Result<(), anyhow::Error> {
        let mut index = self.repo.index()?;
        let tree_id = index.write_tree()?;
        let signature = Signature::now("Package Publisher", "team@exogress.com")?;
        let parent = self
            .repo
            .head()
            .ok()
            .and_then(|h| h.target())
            .ok_or(anyhow::Error::msg("no head"))?;
        let parent = self.repo.find_commit(parent)?;
        let _ = self.repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            msg,
            &self.repo.find_tree(tree_id)?,
            &[&parent],
        )?;

        Ok(())
    }

    pub fn push(&self, github_token: &str) -> Result<(), anyhow::Error> {
        info!("find origin");
        let mut origin = self.repo.find_remote("origin")?;
        info!("push");
        let mut push_options = PushOptions::new();
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|url, username_from_url, allowed_types| {
            info!(
                "credentials callback: {:?}, {:?}, {:?}",
                url, username_from_url, allowed_types
            );
            Cred::userpass_plaintext(github_token, "")
        });

        callbacks.sideband_progress(|t| {
            let s = std::str::from_utf8(t).expect("bad string");
            info!("GIT PROGRESS: {}", s);
            true
        });

        callbacks.push_update_reference(|s1, s2| {
            info!("GIT UPDATE: {:?}, {:?}", s1, s2);
            Ok(())
        });

        push_options.remote_callbacks(callbacks);
        origin.push::<String>(
            &["refs/heads/master:refs/heads/master".to_string()],
            Some(&mut push_options),
        )?;

        Ok(())
    }
}
