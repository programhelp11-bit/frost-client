# Build:  docker build -f server/Dockerfile -t frost-server .
# Run:    docker run -p 8080:8080 -v frost-data:/data -e ADMIN_TOKEN=change-me frost-server
FROM rust:1-slim AS build
WORKDIR /src
COPY server ./server
WORKDIR /src/server
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/server/target/release/frost-server /usr/local/bin/frost-server
ENV DATA_DIR=/data PORT=8080
VOLUME /data
EXPOSE 8080
CMD ["frost-server"]
