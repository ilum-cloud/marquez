{{/*
Create ilum-marquez image pull secrets helper
*/}}
{{- define "ilum-marquez.imagePullSecrets" -}}
{{- $pullSecrets := list -}}

{{- range .Values.marquez.imagePullSecrets -}}
  {{- if kindIs "map" . -}}
    {{- $pullSecrets = append $pullSecrets . -}}
  {{- else -}}
    {{- $pullSecrets = append $pullSecrets (dict "name" .) -}}
  {{- end -}}
{{- end -}}

{{- if (not (empty $pullSecrets)) -}}
{{- $pullSecrets | toYaml -}}
{{- end -}}
{{- end -}}

{{/*
Create ilum-marquez-web image pull secrets helper
*/}}
{{- define "ilum-marquez-web.imagePullSecrets" -}}
{{- $pullSecrets := list -}}

{{- range .Values.web.imagePullSecrets -}}
  {{- if kindIs "map" . -}}
    {{- $pullSecrets = append $pullSecrets . -}}
  {{- else -}}
    {{- $pullSecrets = append $pullSecrets (dict "name" .) -}}
  {{- end -}}
{{- end -}}

{{- if (not (empty $pullSecrets)) -}}
{{- $pullSecrets | toYaml -}}
{{- end -}}
{{- end -}}

{{/*
Expand the name of the chart.
*/}}
{{- define "ilum-marquez.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
We truncate at 63 chars because some Kubernetes name fields are limited to this (by the DNS naming spec).
If release name contains chart name it will be used as a full name.
*/}}
{{- define "ilum-marquez.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "ilum-marquez.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "ilum-marquez.labels" -}}
helm.sh/chart: {{ include "ilum-marquez.chart" . }}
{{ include "ilum-marquez.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- if .Values.commonLabels }}
{{ toYaml .Values.commonLabels }}
{{- end }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "ilum-marquez.selectorLabels" -}}
app.kubernetes.io/name: {{ include "ilum-marquez.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "ilum-marquez.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "ilum-marquez.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Database environment variables (POSTGRES_HOST/PORT/DB/USER) for the Marquez
containers. Each value is read from the existing secret when the matching key
in marquez.existingSecretKeys is set; otherwise it falls back to the bundled
PostgreSQL subchart or the plain values under marquez.db.
*/}}
{{- define "ilum-marquez.databaseEnv" -}}
{{- $keys := default dict .Values.marquez.existingSecretKeys -}}
- name: POSTGRES_HOST
{{- if and .Values.marquez.existingSecretName $keys.hostKey }}
  valueFrom:
    secretKeyRef:
      name: {{ .Values.marquez.existingSecretName }}
      key: {{ $keys.hostKey }}
{{- else if .Values.postgresql.enabled }}
  value: {{ printf "%s-%s" .Release.Name "postgresql" | trunc 63 | trimSuffix "-" | quote }}
{{- else }}
  value: {{ .Values.marquez.db.host | quote }}
{{- end }}
- name: POSTGRES_PORT
{{- if and .Values.marquez.existingSecretName $keys.portKey }}
  valueFrom:
    secretKeyRef:
      name: {{ .Values.marquez.existingSecretName }}
      key: {{ $keys.portKey }}
{{- else if .Values.postgresql.enabled }}
  value: "5432"
{{- else }}
  value: {{ .Values.marquez.db.port | quote }}
{{- end }}
- name: POSTGRES_DB
{{- if and .Values.marquez.existingSecretName $keys.databaseKey }}
  valueFrom:
    secretKeyRef:
      name: {{ .Values.marquez.existingSecretName }}
      key: {{ $keys.databaseKey }}
{{- else if .Values.postgresql.enabled }}
  value: {{ .Values.postgresql.auth.database | quote }}
{{- else }}
  value: {{ .Values.marquez.db.name | quote }}
{{- end }}
- name: POSTGRES_USER
{{- if and .Values.marquez.existingSecretName $keys.userKey }}
  valueFrom:
    secretKeyRef:
      name: {{ .Values.marquez.existingSecretName }}
      key: {{ $keys.userKey }}
{{- else if .Values.postgresql.enabled }}
  value: {{ .Values.postgresql.auth.username | quote }}
{{- else }}
  value: {{ .Values.marquez.db.user | quote }}
{{- end }}
{{- end }}

{{/*
secretKeyRef pointing at the database password.
*/}}
{{- define "ilum-marquez.databasePasswordSecretKeyRef" -}}
{{- $keys := default dict .Values.marquez.existingSecretKeys -}}
secretKeyRef:
{{- if .Values.marquez.existingSecretName }}
  name: {{ .Values.marquez.existingSecretName }}
  key: {{ default "marquez-db-password" $keys.passwordKey }}
{{- else if .Values.postgresql.enabled }}
  name: {{ printf "%s-%s" .Release.Name "postgresql" | trunc 63 | trimSuffix "-" }}
  key: password
{{- else }}
  name: {{ include "ilum-marquez.fullname" . }}
  key: marquez-db-password
{{- end }}
{{- end }}
