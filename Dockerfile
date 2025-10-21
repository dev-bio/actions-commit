FROM rust@sha256:976303ceda00c5f21d6fe97500927285c7e0f6a2e8df71ae18a6c8e9b37550a1

RUN echo "runner:x:1001:121:runner:/home/runner:/sbin/nologin" >> /etc/passwd && \
    echo "runner:x:121:runner" >> /etc/group

USER runner:runner
COPY action action

CMD ["/action"]
