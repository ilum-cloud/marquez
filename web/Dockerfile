FROM node:lts-alpine AS builder
WORKDIR /usr/src/app

ARG REACT_APP_ADVANCED_SEARCH
ENV REACT_APP_ADVANCED_SEARCH=$REACT_APP_ADVANCED_SEARCH

RUN apk add --no-cache git

COPY package*.json ./

RUN npm install

COPY . .

RUN npm run build

FROM node:lts-alpine AS runner
WORKDIR /usr/src/app

RUN apk add --no-cache bash coreutils

RUN npm init -y >/dev/null 2>&1 \
 && npm install express@^5 http-proxy-middleware@^2.0.6

ARG REACT_APP_ADVANCED_SEARCH
ENV REACT_APP_ADVANCED_SEARCH=$REACT_APP_ADVANCED_SEARCH

COPY --from=builder /usr/src/app/dist ./dist
COPY docker/entrypoint.sh ./entrypoint.sh
COPY setupProxy.js /usr/src/app/setupProxy.js

EXPOSE 3000
ENTRYPOINT ["/usr/src/app/entrypoint.sh"]
