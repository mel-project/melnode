FROM summerwind/actions-runner:latest

RUN sudo apt update
RUN sudo apt -y install curl

COPY themelio-node /usr/local/bin/themelio-node
COPY run.sh /usr/local/bin/run.sh

EXPOSE 8080
EXPOSE 11814

ENTRYPOINT ["run.sh"]