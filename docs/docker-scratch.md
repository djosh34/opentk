# Static Scratch Docker Images

Build the multi-architecture static scratch runtime images with:

```bash
scripts/docker-buildx-scratch.sh
```

The script creates or reuses a Docker Buildx builder, builds one local artifact
image containing all four Rust binaries, assembles the final static scratch
runtime images, verifies `--help` on the host architecture, checks the image size
limit, and verifies the `linux/amd64` and `linux/arm64` manifest entries.

The artifact image is built by native Rust cross-compilation, not QEMU or
emulated target execution. It contains:

- `x86_64-unknown-linux-musl/opentk-sync`
- `x86_64-unknown-linux-musl/opentk-api`
- `aarch64-unknown-linux-musl/opentk-sync`
- `aarch64-unknown-linux-musl/opentk-api`

Direct final-image builds use the prebuilt artifact image:

```bash
docker buildx build --platform linux/amd64,linux/arm64 -f docker/Dockerfile.scratch --build-arg BINARY=opentk-sync .
docker buildx build --platform linux/amd64,linux/arm64 -f docker/Dockerfile.scratch --build-arg BINARY=opentk-api .
```

The final images are static scratch runtime images. Each platform image contains
only the selected binary at `/bin/opentk`, CA certificates, minimal passwd/group
identity files for uid/gid `1000`, and no runtime distro shell or package
manager.
