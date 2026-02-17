<h1 align="center"> Hyperion </h1>

<div align="center">
    <img src="https://img.shields.io/github/license/GrandmasterB42/hyperion?style=for-the-badge&color=rgb(134,1,175)" alt="License">
    <img src="https://img.shields.io/github/languages/code-size/GrandmasterB42/hyperion?style=for-the-badge&color=rgb(134,1,175)" alt="Code Size">
    <img src="https://www.aschey.tech/tokei/github.com/GrandmasterB42/hyperion?style=for-the-badge&color=rgb(134,1,175)" alt="Lines of Code">
    <img src="https://img.shields.io/badge/language-Rust-orange?style=for-the-badge&color=rgb(134,1,175)" alt="Language">
</div>

> [!WARNING]
> Hyperion is in early development. Expect breaking changes, missing features, and rough edges.

Hyperion is aiming to be a Minecraft server framework that enables massive multiplayer experiences by offloading tasks to proxy servers working in conjunction with the main game server. Built on top of [Bevy ECS](https://bevy.org/), written in [Rust](https://www.rust-lang.org/).

Currently targets **Minecraft 1.20.1**. Keeping up with newer Versions is a goal, but building a solid foundation is the priority.

## Acknowledgements[^1]

- [Valence](https://github.com/valence-rs/valence) — Hyperion currently builds on Valence for protocol types and tooling.
- [FerrumC](https://github.com/ferrumc-rs/ferrumc) — A big inspiration for the project restructuring, might become a dependency in the future.
- [Bevy](https://bevyengine.org/) — A lot of this project runs on the Bevy ECS, allowing for a parallelized architecture.
- [Minecraft Protocol Wiki](https://minecraft.wiki/w/Java_Edition_protocol) — An invaluable resource for understanding the Minecraft protocol.

[^1]: in no particular order

## How to Use

This project is currently not easily usable unless you are a developer and somewhat familiar with the concepts. Any contributions, particularly to improve the documentation and usability, are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) if this applies to you.

It is also not meant to be a plug and play solution. Plugin support is currently not planned to be dynamic in any way and will porbably always need you to write Rust code and compile a project yourself to get things running like you want.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, building from source, architecture details, and how to get involved.

- **Issues**: [GitHub Issues](https://github.com/GrandmasterB42/hyperion/issues)
