FROM rust as builder

WORKDIR /opt/auto-invite-matrix-bot
COPY . .
RUN cargo install --path .

FROM debian:buster-slim
COPY --from=builder /usr/local/cargo/bin/auto-invite-matrix-bot /usr/local/bin/auto-invite-matrix-bot
RUN apt-get update && apt-get install -y libssl1.1 ca-certificates && rm -rf /var/lib/apt/lists/*

CMD ["auto-invite-matrix-bot"]
