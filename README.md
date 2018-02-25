# Terrain generator

[![Latest Github release](https://img.shields.io/github/release/fabio-t/terrain-generator.svg)](https://github.com/fabio-t/terrain-generator/releases/latest)

Minimal terrain generator based on OpenSimplexNoise. It supports island shapes, allows river creation and
makes it simple to configure the octaved (or "fractal") noise generation for heightmaps, moisture and so on.

It ships with a complete example (just run the Main and inspect the images generated, then play around with parameters).

When and if it will become more complex, I'll write better documentation. Right now, you are going to play around
with the parameters yourself.

Feel free to write an Issue if you have questions or need help.

## Install

1. First add Jitpack as a repository inside your pom.xml

```xml
<repositories>
  <repository>
    <id>jitpack.io</id>
    <url>https://jitpack.io</url>
  </repository>
</repositories>
```

2. Then add `rlforj-alt` as a dependency (make sure to change `version` to one of the
[releases](https://github.com/fabio-t/terrain-generator/releases)):

```xml
<dependency>
    <groupId>com.github.fabio-t</groupId>
    <artifactId>terrain-generator</artifactId>
    <version>version</version>
</dependency>
```
