# Release Management

Currently, Codex Cloud binaries are distributed in three places:

- GitHub Releases https://github.com/cklxx/codex-cloud/releases
- `codex-cloud-cli` on npm: https://www.npmjs.com/package/codex-cloud-cli
- `codex-cloud` on Homebrew (tap: `cklxx/tools`)

# Cutting a Release

Run the `codex-rs/scripts/create_github_release` script in the repository to publish a new release. The script will choose the appropriate version number depending on the type of release you are creating.

To cut a new alpha release from `main` (feel free to cut alphas liberally):

```
./codex-rs/scripts/create_github_release --publish-alpha
```

To cut a new _public_ release from `main` (which requires more caution), run:

```
./codex-rs/scripts/create_github_release --publish-release
```

TIP: Add the `--dry-run` flag to report the next version number for the respective release and exit.

Running the publishing script will kick off a GitHub Action to build the release, so go to https://github.com/cklxx/codex-cloud/actions/workflows/rust-release.yml to find the corresponding workflow. (Note: we should automate finding the workflow URL with `gh`.)

When the workflow finishes, the GitHub Release is "done," but you still have to consider npm and Homebrew.

## Publishing to npm

The GitHub Action is responsible for publishing to npm.

## Publishing to Homebrew

For Homebrew, we publish through the personal tap `cklxx/tools`. After cutting a release, bump the formula by running:

```
brew tap cklxx/tools
brew bump-formula-pr cklxx/tools/codex-cloud --version <version>
```

Once CI finishes, merge the tap PR to make the new bottle available.
