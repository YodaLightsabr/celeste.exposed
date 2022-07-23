FROM rust:1.61.0

WORKDIR /usr/src/celeste
COPY . .

RUN apt-get update
RUN apt-get install -y certbot
RUN certbot certonly --standalone -d celeste.exposed -m "parafactual@gmail.com" --non-interactive --agree-tos

RUN apt-get update
RUN apt-get install -y npm
RUN npm install -g browserify

RUN make

CMD ["cargo", "run", "--release"]