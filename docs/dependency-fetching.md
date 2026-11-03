## Gradle Dependency Fetching

Kost can fetch dependencies in gradle files. It supports support JARs, AARs, and source JARs.

## Usages

#### Fetch

```bash
kost dep fetch # Fetch all resolved project dependencies 
kost dep fetch --sources # Fetch with source jars
kost dep fetch androidx.compose.ui:ui:1.10.0 # Fetch a specific dependency
```

#### List

```bash
kost dep list # List resolved project dependencies
kost dep list --offline # Resolve only cached dependencies
```

#### Tree

```bash
kost dep tree # Print resolved project dependencies as a tree
kost dep tree --offline # Resolve only cached dependencies
```

## Gradle Home

Gradle home is resolved in the following order:

1. `--gradle-home <PATH>` CLI option
2. `GRADLE_USER_HOME` environment variable
3. `~/.gradle`

## Downloading

Set `KOST_CONCURRENT_DOWNLOADS` environment variable to tweak max concurrent downloads. Defaults to `32`.
