FROM registry.fedoraproject.org/fedora:44 AS build
RUN dnf install -y rust cargo pkgconf-pkg-config \
      wayland-devel wayland-protocols-devel libxkbcommon-devel \
      mesa-libEGL-devel mesa-libgbm-devel vulkan-loader-devel \
      gstreamer1-devel gstreamer1-plugins-base-devel gstreamer1-rtsp-server-devel \
    && dnf clean all
WORKDIR /src
COPY . .
RUN cargo build --release --locked

FROM registry.fedoraproject.org/fedora:44
# x264enc is not in Fedora proper; it comes from RPM Fusion free's gstreamer1-plugins-ugly.
RUN dnf install -y https://mirrors.rpmfusion.org/free/fedora/rpmfusion-free-release-44.noarch.rpm \
    && dnf install -y gstreamer1 gstreamer1-plugins-base gstreamer1-plugins-good \
      gstreamer1-plugins-bad-free gstreamer1-plugins-ugly-free gstreamer1-plugins-ugly gstreamer1-rtsp-server \
      libxkbcommon libwayland-client \
    && dnf clean all
RUN gst-inspect-1.0 x264enc
COPY --from=build /src/target/release/hikvision-thermal-wayland-viewer /usr/local/bin/
COPY vendor/linux/x86_64/hik_sdk /opt/hik_sdk
WORKDIR /app
ENTRYPOINT ["/usr/local/bin/hikvision-thermal-wayland-viewer", "--sdk-root", "/opt/hik_sdk"]
