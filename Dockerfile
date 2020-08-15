FROM rust as builder

WORKDIR /opt/auto-invite-matrix-bot
COPY . .
RUN cargo install --path .

FROM debian:buster-slim
COPY --from=builder /usr/local/cargo/bin/auto-invite-matrix-bot /usr/local/bin/auto-invite-matrix-bot

CMD ["auto-invite-matrix-bot", "--config /data/config.yaml"]
