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
