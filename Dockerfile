FROM alpine@sha256:4b7ce07002c69e8f3d704a9c5d6fd3053be500b7f1c69fc0d80990c2ad8dd412

RUN echo "runner:x:1001:121:runner:/home/runner:/sbin/nologin" >> /etc/passwd && \
    echo "runner:x:121:runner" >> /etc/group

USER runner:runner
COPY action action

CMD ["/action"]
