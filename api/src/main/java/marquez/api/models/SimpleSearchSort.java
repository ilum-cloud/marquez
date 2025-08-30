/*
 * Copyright 2018-2023 contributors to the Marquez project
 * SPDX-License-Identifier: Apache-2.0
 */

package marquez.api.models;

/** Sort options supported for simple search results. */
public enum SimpleSearchSort {
  NAME("name"),
  UPDATED_AT("updated_at");

  final String value;

  SimpleSearchSort(String value) {
    this.value = value;
  }

  public String getValue() {
    return value;
  }
}
