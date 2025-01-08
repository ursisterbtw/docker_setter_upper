
# Generated Dockerfile
FROM rust:1.83-slim
LABEL maintainer="Generated <generated@example.com>"
RUN apt-get update && apt-get install -y \
    cargo \
WORKDIR /app
ENTRYPOINT ["/bin/bash"]
