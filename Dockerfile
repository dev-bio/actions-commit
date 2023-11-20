FROM ghcr.io/dev-bio/native-action-base:latest

COPY action action

CMD ["/action"]
