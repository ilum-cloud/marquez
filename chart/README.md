# ilum-marquez packaged by ilum

## TL;DR

```bash
$ helm repo add ilum https://charts.ilum.cloud
$ helm install ilum-marquez ilum/ilum-marquez
```

## Installing the Chart

To install the chart with the release name `ilum-marquez`:

```bash
$ helm install ilum-marquez ilum/ilum-marquez --set postgresql.enabled=true
```

The command deploys `ilum-marquez` on the Kubernetes cluster in the default configuration. The [Parameters](#parameters)
section lists the parameters that can be configured during installation.

## Uninstalling the Chart

To uninstall/delete the `ilum-marquez` deployment:

```bash
$ helm delete ilum-marquez
```

The command removes all the Kubernetes components associated with the chart and deletes the release.

## Parameters

### Marquez parameters

| Parameter                     | Description                                          | Default                                |
|-------------------------------|------------------------------------------------------|----------------------------------------|
| `marquez.replicaCount`        | Number of desired replicas                           | `1`                                    |
| `marquez.image.registry`      | Image registry                                       | `docker.io`                            |
| `marquez.image.repository`    | Image repository                                     | `ilum/marquez`                         |
| `marquez.image.tag`           | Image tag                                            | `0.53.1`                               |
| `marquez.image.pullPolicy`    | Image pull policy                                    | `IfNotPresent`                         |
| `marquez.existingSecretName`  | Name of existing secret for DB credentials           | `""`                                   |
| `marquez.extraContainers`     | Sidecar containers to add to the Marquez pod         | `[]`                                   |
| `marquez.pdb.create`          | Create PodDisruptionBudget                           | `false`                                |
| `marquez.podSecurityContext`  | Pod security context                                 | `{}`                                   |
| `marquez.securityContext`     | Container security context                           | `{}`                                   |
| `marquez.db.host`             | PostgreSQL host (ignored if postgresql.enabled=true) | `ilum-postgresql-0.ilum-postgresql-hl` |
| `marquez.db.port`             | PostgreSQL port                                      | `5432`                                 |
| `marquez.db.name`             | PostgreSQL database                                  | `marquez`                              |
| `marquez.db.user`             | PostgreSQL user                                      | `ilum`                                 |
| `marquez.db.password`         | PostgreSQL password                                  | `CHANGEMEPLEASE`                       |
| `marquez.dbRetention.enabled` | Enable DB retention policy                           | `false`                                |
| `marquez.migrateOnStartup`    | Execute Flyway migration                             | `true`                                 |
| `marquez.resources`           | Resource limits/requests                             | `{}`                                   |

### Marquez service parameters

| Parameter                        | Description                    | Default     |
|----------------------------------|--------------------------------|-------------|
| `marquez.service.type`           | Marquez service type           | `ClusterIP` |
| `marquez.service.port`           | Marquez service port           | `9555`      |
| `marquez.service.nodePort`       | Marquez service nodePort       | `""`        |
| `marquez.service.loadBalancerIP` | Marquez service loadBalancerIP | `""`        |
| `marquez.service.annotations`    | Marquez service annotations    | `{}`        |

### Marquez Web UI parameters

| Parameter                | Description                | Default            |
|--------------------------|----------------------------|--------------------|
| `web.enabled`            | Enables creation of Web UI | `true`             |
| `web.image.registry`     | Image registry             | `docker.io`        |
| `web.image.repository`   | Image repository           | `ilum/marquez-web` |
| `web.image.tag`          | Image tag                  | `0.53.1`           |
| `web.podSecurityContext` | Pod security context       | `{}`               |
| `web.securityContext`    | Container security context | `{}`               |
| `web.pdb.create`         | Create PodDisruptionBudget | `false`            |
| `web.resources`          | Resource limits/requests   | `{}`               |

### Marquez web service parameters

| Parameter                    | Description                    | Default     |
|------------------------------|--------------------------------|-------------|
| `web.service.type`           | Marquez service type           | `ClusterIP` |
| `web.service.port`           | Marquez service port           | `9444`      |
| `web.service.nodePort`       | Marquez service nodePort       | `""`        |
| `web.service.loadBalancerIP` | Marquez service loadBalancerIP | `""`        |
| `web.service.annotations`    | Marquez service annotations    | `{}`        |

### PostgreSQL parameters

| Parameter                  | Description                | Default          |
|----------------------------|----------------------------|------------------|
| `postgresql.enabled`       | Deploy PostgreSQL subchart | `false`          |
| `postgresql.auth.username` | PostgreSQL username        | `ilum`           |
| `postgresql.auth.password` | PostgreSQL password        | `CHANGEMEPLEASE` |
| `postgresql.auth.database` | PostgreSQL database        | `marquez`        |

### Other parameters

| Parameter               | Description                           | Default |
|-------------------------|---------------------------------------|---------|
| `commonLabels`          | Labels to apply to all resources      | `{}`    |
| `commonAnnotations`     | Annotations to apply to all resources | `{}`    |
| `serviceAccount.create` | Create ServiceAccount                 | `true`  |
| `serviceAccount.name`   | ServiceAccount name to use            | `""`    |
| `ingress.enabled`       | Enable Ingress                        | `false` |

## Local Installation Guide

### Helm Managed Postgres

The quickest way to install Marquez via Kubernetes is to create a local Postgres instance.

```bash
helm install ilum-marquez . --dependency-update --set postgresql.enabled=true
```

### Docker Postgres

A Postgres database is configured within the Marquez project that use Docker to launch, which provides the added
benefit of test data seeding. You can run the following command to create this instance of Postgres via Docker.
Contents of the ```./../docker-compose-postgres..yml``` file can be customized
to better represent your desired setup.

```bash
docker-compose -f ./../docker-compose.db.yml -p marquez-postgres up
```

Once the Postgres instance has been created, run the following command to locate the IP
address of the database. Note you will need to un-escape the markdown.

```bash
marquez_db_ip=$(docker inspect marquez-postgres_db_1 -f '{{range.NetworkSettings.Networks}}{{.Gateway}}{{end}}')
```

Deploy via Helm and update database values as needed, either via
the `values.yaml` file or within the Helm CLI command. Again, remove the
pesky markdown escape character before running this command.

```bash
helm install ilum-marquez . --dependency-update --set marquez.db.host=$marquez_db_ip
```

### Validation

Once the Kubernetes pods and services have been installed (usually within 5-10 seconds), connectivity
tests can be executed by running the following Helm command. You should see a status message
of `Succeeded` for each test if the HTTP endpoints were reachable.

```bash
helm test ilum-marquez
```

If you haven't configured ingress within the Helm chart values, then you can use the
following port forwarding rules to support local development.

```bash
kubectl port-forward svc/ilum-marquez 5000:80
```

```bash
kubectl port-forward svc/ilum-marquez-web 3000:80
```

Once these rules are in place, you can view both the APIs and UI using the
links below.
* http://localhost:5000/api/v1/namespaces
* http://localhost:3000

### Troubleshooting
If things aren't working as expected, you can find out more by viewing the `kubectl` logs.
First, get the name of the pod that was installed by the Helm chart.

```bash
kubectl get pods
```

Plug this pod name into the following command, and it will display logs related
to the database migrations. This makes it simple to see errors dealing with
networking issues, credentials, etc.

```bash
kubectl logs -p <podName>
```