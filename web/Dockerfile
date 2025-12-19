FROM node:18-alpine AS deps
WORKDIR /usr/src/app

RUN apk add --no-cache git

COPY package*.json ./
COPY libs ./libs

RUN npm install

COPY . .

ARG REACT_APP_ADVANCED_SEARCH
ENV REACT_APP_ADVANCED_SEARCH=$REACT_APP_ADVANCED_SEARCH
RUN npm run build

FROM node:24-alpine AS release
WORKDIR /usr/src/app

RUN npm install express@4 http-proxy-middleware@2

COPY --from=deps /usr/src/app/dist ./dist
COPY setupProxy.js ./
COPY docker/entrypoint.sh ./entrypoint.sh

RUN chmod +x ./entrypoint.sh

EXPOSE 3000
ENTRYPOINT ["./entrypoint.sh"]
