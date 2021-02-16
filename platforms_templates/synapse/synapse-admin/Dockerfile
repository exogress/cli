FROM ubuntu

COPY --from=awesometechnologies/synapse-admin /app /app

RUN apt update && apt install -y gnupg curl && \
    curl -s https://apt.exogress.com/KEY.gpg | apt-key add - && \
    echo "deb https://apt.exogress.com/ /" > /etc/apt/sources.list.d/exogress.list && \
    apt update && apt install -y exogress

ADD Exofile.yml .

CMD ["exogress", "spawn"]
