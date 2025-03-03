<!-- Copyright 2023 Zinc Labs Inc.

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU Affero General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU Affero General Public License for more details.

You should have received a copy of the GNU Affero General Public License
along with this program.  If not, see <http://www.gnu.org/licenses/>.
-->

<template>
  <div
    class="column index-menu full-height"
    :class="store.state.theme == 'dark' ? 'theme-dark' : 'theme-light'"
  >
    <div>
      <q-select
        data-test="log-search-index-list-select-stream"
        v-model="searchObj.data.stream.selectedStream"
        :label="
          searchObj.data.stream.selectedStream.label
            ? ''
            : t('search.selectIndex')
        "
        :options="streamOptions"
        data-cy="index-dropdown"
        input-debounce="0"
        behavior="menu"
        filled
        borderless
        dense
        use-input
        hide-selected
        fill-input
        @filter="filterStreamFn"
        @update:model-value="onStreamChange"
      >
        <template #no-option>
          <q-item>
            <q-item-section> {{ t("search.noResult") }}</q-item-section>
          </q-item>
        </template>
      </q-select>
    </div>
    <div class="index-table q-mt-xs">
      <q-table
        data-test="log-search-index-list-fields-table"
        v-model="searchObj.data.stream.selectedFields"
        :visible-columns="['name']"
        :rows="searchObj.data.stream.selectedStreamFields"
        :row-key="
          (row) => searchObj.data.stream.selectedStream.label + row.name
        "
        :filter="searchObj.data.stream.filterField"
        :filter-method="filterFieldFn"
        :pagination="{ rowsPerPage: 10000 }"
        hide-header
        hide-bottom
        :wrap-cells="searchObj.meta.resultGrid.wrapCells"
        class="field-table full-height"
        id="fieldList"
      >
        <template #body-cell-name="props">
          <q-tr :props="props">
            <q-td
              :props="props"
              class="field_list"
              :class="
                searchObj.data.stream.selectedFields.includes(props.row.name)
                  ? 'selected'
                  : ''
              "
            >
              <!-- TODO OK : Repeated code make seperate component to display field  -->
              <div
                v-if="props.row.ftsKey"
                class="field-container flex content-center ellipsis q-pl-lg q-pr-sm"
                :title="props.row.name"
              >
                <div
                  class="field_label ellipsis"
                  :data-test="`logs-field-list-item-${props.row.name}`"
                >
                  {{ props.row.name }}
                </div>
                <div class="field_overlay">
                  <q-btn
                    :icon="outlinedAdd"
                    :data-test="`log-search-index-list-filter-${props.row.name}-field-btn`"
                    style="margin-right: 0.375rem"
                    size="0.4rem"
                    class="q-mr-sm"
                    @click.stop="addToFilter(`${props.row.name}=''`)"
                    round
                  />
                  <q-icon
                    :data-test="`log-search-index-list-add-${props.row.name}-field-btn`"
                    v-if="
                      !searchObj.data.stream.selectedFields.includes(
                        props.row.name
                      )
                    "
                    :name="outlinedVisibility"
                    size="1.1rem"
                    title="Add field to table"
                    @click.stop="clickFieldFn(props.row, props.pageIndex)"
                  />
                  <q-icon
                    :data-test="`log-search-index-list-remove-${props.row.name}-field-btn`"
                    v-if="
                      searchObj.data.stream.selectedFields.includes(
                        props.row.name
                      )
                    "
                    :name="outlinedVisibilityOff"
                    size="1.1rem"
                    title="Remove field from table"
                    @click.stop="clickFieldFn(props.row, props.pageIndex)"
                  />
                </div>
              </div>
              <q-expansion-item
                v-else
                dense
                switch-toggle-side
                :label="props.row.name"
                expand-icon-class="field-expansion-icon"
                expand-icon="
                  expand_more
                "
                expanded-icon="
                  expand_less
                "
                @before-show="(event: any) => openFilterCreator(event, props.row)"
              >
                <template v-slot:header>
                  <div
                    class="flex content-center ellipsis"
                    :title="props.row.name"
                    :data-test="`log-search-expand-${props.row.name}-field-btn`"
                  >
                    <div
                      class="field_label ellipsis"
                      :data-test="`logs-field-list-item-${props.row.name}`"
                    >
                      {{ props.row.name }}
                    </div>
                    <div class="field_overlay">
                      <q-btn
                        :data-test="`log-search-index-list-filter-${props.row.name}-field-btn`"
                        :icon="outlinedAdd"
                        style="margin-right: 0.375rem"
                        size="0.4rem"
                        class="q-mr-sm"
                        @click.stop="addToFilter(`${props.row.name}=''`)"
                        round
                      />
                      <q-icon
                        :data-test="`log-search-index-list-add-${props.row.name}-field-btn`"
                        v-if="
                          !searchObj.data.stream.selectedFields.includes(
                            props.row.name
                          )
                        "
                        :name="outlinedVisibility"
                        size="1.1rem"
                        title="Add field to table"
                        @click.stop="clickFieldFn(props.row, props.pageIndex)"
                      />
                      <q-icon
                        :data-test="`log-search-index-list-remove-${props.row.name}-field-btn`"
                        v-if="
                          searchObj.data.stream.selectedFields.includes(
                            props.row.name
                          )
                        "
                        :name="outlinedVisibilityOff"
                        title="Remove field from table"
                        size="1.1rem"
                        @click.stop="clickFieldFn(props.row, props.pageIndex)"
                      />
                    </div>
                  </div>
                </template>
                <q-card>
                  <q-card-section class="q-pl-md q-pr-xs q-py-xs">
                    <div class="filter-values-container">
                      <div
                        v-show="fieldValues[props.row.name]?.isLoading"
                        class="q-pl-md q-py-xs"
                        style="height: 60px"
                      >
                        <q-inner-loading
                          size="xs"
                          :showing="fieldValues[props.row.name]?.isLoading"
                          label="Fetching values..."
                          label-style="font-size: 1.1em"
                        />
                      </div>
                      <div
                        v-show="
                          !fieldValues[props.row.name]?.values?.length &&
                          !fieldValues[props.row.name]?.isLoading
                        "
                        class="q-pl-md q-py-xs text-subtitle2"
                      >
                        No values found
                      </div>
                      <div
                        v-for="value in fieldValues[props.row.name]?.values ||
                        []"
                        :key="value.key"
                      >
                        <q-list dense>
                          <q-item
                            tag="label"
                            class="q-pr-none"
                            :data-test="`logs-search-subfield-add-${props.row.name}-${value.key}`"
                          >
                            <div
                              class="flex row wrap justify-between"
                              style="width: calc(100% - 42px)"
                            >
                              <div
                                :title="value.key"
                                class="ellipsis q-pr-xs"
                                style="width: calc(100% - 50px)"
                              >
                                {{ value.key }}
                              </div>
                              <div
                                :title="value.count"
                                class="ellipsis text-right q-pr-sm"
                                style="width: 50px"
                              >
                                {{ value.count }}
                              </div>
                            </div>
                            <div
                              class="flex row"
                              :class="
                                store.state.theme === 'dark'
                                  ? 'text-white'
                                  : 'text-black'
                              "
                            >
                              <q-btn
                                class="q-mr-xs"
                                size="6px"
                                @click="
                                  addSearchTerm(
                                    `${props.row.name}='${value.key}'`
                                  )
                                "
                                title="Include Term"
                                round
                                :data-test="`log-search-subfield-list-equal-${props.row.name}-field-btn`"
                              >
                                <q-icon>
                                  <EqualIcon></EqualIcon>
                                </q-icon>
                              </q-btn>
                              <q-btn
                                size="6px"
                                @click="
                                  addSearchTerm(
                                    `${props.row.name}!='${value.key}'`
                                  )
                                "
                                title="Exclude Term"
                                round
                                :data-test="`log-search-subfield-list-not-equal-${props.row.name}-field-btn`"
                              >
                                <q-icon>
                                  <NotEqualIcon></NotEqualIcon>
                                </q-icon>
                              </q-btn>
                            </div>
                          </q-item>
                        </q-list>
                      </div>
                    </div>
                  </q-card-section>
                </q-card>
              </q-expansion-item>
            </q-td>
          </q-tr>
        </template>
        <template #top-right>
          <q-input
            data-test="log-search-index-list-field-search-input"
            v-model="searchObj.data.stream.filterField"
            data-cy="index-field-search-input"
            filled
            borderless
            dense
            clearable
            debounce="1"
            :placeholder="t('search.searchField')"
          >
            <template #prepend>
              <q-icon name="search" />
            </template>
          </q-input>
        </template>
      </q-table>
    </div>
  </div>
</template>

<script lang="ts">
import { defineComponent, ref, type Ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useStore } from "vuex";
import { useQuasar } from "quasar";
import { useRouter } from "vue-router";
import useLogs from "../../composables/useLogs";
import {
  b64EncodeUnicode,
  getImageURL,
  convertTimeFromMicroToMilli,
  formatLargeNumber,
} from "../../utils/zincutils";
import streamService from "../../services/stream";
import { Parser } from "node-sql-parser/build/mysql";
import {
  outlinedAdd,
  outlinedVisibility,
  outlinedVisibilityOff,
} from "@quasar/extras/material-icons-outlined";
import EqualIcon from "@/components/icons/EqualIcon.vue";
import NotEqualIcon from "@/components/icons/NotEqualIcon.vue";
import { getConsumableRelativeTime } from "@/utils/date";
import { cloneDeep } from "lodash-es";

interface Filter {
  fieldName: string;
  selectedValues: string[];
  selectedOperator: string;
}
export default defineComponent({
  name: "ComponentSearchIndexSelect",
  components: { EqualIcon, NotEqualIcon },
  setup() {
    const store = useStore();
    const router = useRouter();
    const { t } = useI18n();
    const $q = useQuasar();
    const {
      searchObj,
      updatedLocalLogFilterField,
      handleQueryData,
      onStreamChange,
    } = useLogs();
    const streamOptions: any = ref(searchObj.data.stream.streamLists);
    const fieldValues: Ref<{
      [key: string | number]: {
        isLoading: boolean;
        values: { key: string; count: string }[];
      };
    }> = ref({});
    const parser = new Parser();

    const streamTypes = [
      { label: t("search.logs"), value: "logs" },
      { label: t("search.enrichmentTables"), value: "enrichment_tables" },
    ];

    const filterStreamFn = (val: string, update: any) => {
      update(() => {
        streamOptions.value = searchObj.data.stream.streamLists;
        const needle = val.toLowerCase();
        streamOptions.value = streamOptions.value.filter(
          (v: any) => v.label.toLowerCase().indexOf(needle) > -1
        );
      });
    };

    watch(
      () => {
        searchObj.data.stream.streamLists.length;
        store.state.organizationData.streams;
      },
      () => {
        streamOptions.value =
          searchObj.data.stream.streamLists ||
          store.state.organizationData.streams;
      }
    );

    const filterFieldFn = (rows: any, terms: any) => {
      var filtered = [];
      if (terms != "") {
        terms = terms.toLowerCase();
        for (var i = 0; i < rows.length; i++) {
          if (rows[i]["name"].toLowerCase().includes(terms)) {
            filtered.push(rows[i]);
          }
        }
      }
      return filtered;
    };

    const addToFilter = (field: any) => {
      searchObj.data.stream.addToFilter = field;
    };

    function clickFieldFn(row: { name: never }, pageIndex: number) {
      if (searchObj.data.stream.selectedFields.includes(row.name)) {
        searchObj.data.stream.selectedFields =
          searchObj.data.stream.selectedFields.filter(
            (v: any) => v !== row.name
          );
      } else {
        searchObj.data.stream.selectedFields.push(row.name);
      }
      searchObj.organizationIdetifier =
        store.state.selectedOrganization.identifier;
      updatedLocalLogFilterField();
    }

    const openFilterCreator = (event: any, { name, ftsKey }: any) => {
      if (ftsKey) {
        event.stopPropagation();
        event.preventDefault();
        return;
      }

      let timestamps: any =
        searchObj.data.datetime.type === "relative"
          ? getConsumableRelativeTime(
              searchObj.data.datetime.relativeTimePeriod
            )
          : cloneDeep(searchObj.data.datetime);

      if (searchObj.data.stream.streamType === "enrichment_tables") {
        const stream = searchObj.data.streamResults.list.find(
          (stream: any) =>
            stream.name === searchObj.data.stream.selectedStream.value
        );
        if (stream.stats) {
          timestamps = {
            startTime:
              new Date(
                convertTimeFromMicroToMilli(
                  stream.stats.doc_time_min - 300000000
                )
              ).getTime() * 1000,
            endTime:
              new Date(
                convertTimeFromMicroToMilli(
                  stream.stats.doc_time_max + 300000000
                )
              ).getTime() * 1000,
          };
        }
      }

      const startISOTimestamp: number = timestamps?.startTime || 0;
      const endISOTimestamp: number = timestamps?.endTime || 0;

      fieldValues.value[name] = {
        isLoading: true,
        values: [],
      };
      try {
        let query_context = "";
        let query = searchObj.data.query;
        if (searchObj.meta.sqlMode == true) {
          const parsedSQL: any = parser.astify(query);
          //hack add time stamp column to parsedSQL if not already added
          query_context =
            b64EncodeUnicode(parser.sqlify(parsedSQL).replace(/`/g, '"')) || "";
        } else {
          let parseQuery = query.split("|");
          let queryFunctions = "";
          let whereClause = "";
          if (parseQuery.length > 1) {
            queryFunctions = "," + parseQuery[0].trim();
            whereClause = parseQuery[1].trim();
          } else {
            whereClause = parseQuery[0].trim();
          }

          query_context =
            `SELECT *${queryFunctions} FROM "` +
            searchObj.data.stream.selectedStream.value +
            `" [WHERE_CLAUSE]`;

          if (whereClause.trim() != "") {
            whereClause = whereClause
              .replace(/=(?=(?:[^"']*"[^"']*"')*[^"']*$)/g, " =")
              .replace(/>(?=(?:[^"']*"[^"']*"')*[^"']*$)/g, " >")
              .replace(/<(?=(?:[^"']*"[^"']*"')*[^"']*$)/g, " <");

            whereClause = whereClause
              .replace(/!=(?=(?:[^"']*"[^"']*"')*[^"']*$)/g, " !=")
              .replace(/! =(?=(?:[^"']*"[^"']*"')*[^"']*$)/g, " !=")
              .replace(/< =(?=(?:[^"']*"[^"']*"')*[^"']*$)/g, " <=")
              .replace(/> =(?=(?:[^"']*"[^"']*"')*[^"']*$)/g, " >=");

            const parsedSQL = whereClause.split(" ");
            searchObj.data.stream.selectedStreamFields.forEach((field: any) => {
              parsedSQL.forEach((node: any, index: any) => {
                if (node == field.name) {
                  node = node.replaceAll('"', "");
                  parsedSQL[index] = '"' + node + '"';
                }
              });
            });

            whereClause = parsedSQL.join(" ");

            query_context = query_context.replace(
              "[WHERE_CLAUSE]",
              " WHERE " + whereClause
            );
          } else {
            query_context = query_context.replace("[WHERE_CLAUSE]", "");
          }
          query_context = b64EncodeUnicode(query_context) || "";
        }

        let query_fn = "";
        if (
          searchObj.data.tempFunctionContent != "" &&
          searchObj.meta.toggleFunction
        ) {
          query_fn = b64EncodeUnicode(searchObj.data.tempFunctionContent) || "";
        }
        streamService
          .fieldValues({
            org_identifier: store.state.selectedOrganization.identifier,
            stream_name: searchObj.data.stream.selectedStream.value,
            start_time: startISOTimestamp,
            end_time: endISOTimestamp,
            fields: [name],
            size: 10,
            query_context: query_context,
            query_fn: query_fn,
          })
          .then((res: any) => {
            if (res.data.hits.length) {
              fieldValues.value[name]["values"] = res.data.hits
                .find((field: any) => field.field === name)
                .values.map((value: any) => {
                  return {
                    key: value.zo_sql_key ? value.zo_sql_key : "null",
                    count: formatLargeNumber(value.zo_sql_num),
                  };
                });
            }
          })
          .finally(() => {
            fieldValues.value[name]["isLoading"] = false;
          });
      } catch (err) {
        $q.notify({
          type: "negative",
          message: "Error while fetching field values",
        });
      }
    };

    const addSearchTerm = (term: string) => {
      // searchObj.meta.showDetailTab = false;
      searchObj.data.stream.addToFilter = term;
    };

    // const onStreamChange = () => {
    //   alert("onStreamChange")
    //   const query = searchObj.meta.sqlMode
    //     ? `SELECT * FROM "${searchObj.data.stream.selectedStream.value}"`
    //     : "";

    //   searchObj.data.editorValue = query;
    //   searchObj.data.query = query;

    //   handleQueryData();
    // };

    return {
      t,
      store,
      router,
      searchObj,
      streamOptions,
      filterFieldFn,
      addToFilter,
      clickFieldFn,
      getImageURL,
      filterStreamFn,
      openFilterCreator,
      addSearchTerm,
      fieldValues,
      streamTypes,
      outlinedAdd,
      outlinedVisibilityOff,
      outlinedVisibility,
      handleQueryData,
      onStreamChange,
    };
  },
});
</script>

<style lang="scss" scoped>
$streamSelectorHeight: 44px;
.q-menu {
  box-shadow: 0px 3px 15px rgba(0, 0, 0, 0.1);
  transform: translateY(0.5rem);
  border-radius: 0px;

  .q-virtual-scroll__content {
    padding: 0.5rem;
  }
}

.index-menu {
  width: 100%;

  .q-field {
    &__control {
      height: 35px;
      padding: 0px 5px;
      min-height: auto !important;

      &-container {
        padding-top: 0px !important;
      }
    }

    &__native :first-of-type {
      padding-top: 0.25rem;
    }
  }

  .index-table {
    width: 100%;
    height: calc(100% - $streamSelectorHeight);
    // border: 1px solid rgba(0, 0, 0, 0.02);

    .q-table {
      display: table;
      table-layout: fixed !important;
    }

    tr {
      margin-bottom: 1px;
    }

    tbody,
    tr,
    td {
      width: 100%;
      display: block;
      height: fit-content;
      overflow: hidden;
    }

    .q-table__control,
    label.q-field {
      width: 100%;
    }

    .q-table thead tr,
    .q-table tbody td {
      height: auto;
    }

    .q-table__top {
      border-bottom: unset;
    }
  }

  .field-table {
    width: 100%;
  }

  .field_list {
    padding: 0px;
    margin-bottom: 0.125rem;
    position: relative;
    overflow: visible;
    cursor: default;

    .field_label {
      pointer-events: none;
      font-size: 0.825rem;
      position: relative;
      display: inline;
      z-index: 2;
      left: 0;
      // text-transform: capitalize;
    }

    .field-container {
      height: 25px;
    }

    .field_overlay {
      position: absolute;
      height: 100%;
      right: 0;
      top: 0;
      z-index: 5;
      padding: 0 6px;
      visibility: hidden;
      display: flex;
      align-items: center;

      .q-icon {
        cursor: pointer;
        opacity: 0;
        margin: 0 1px;
      }
    }

    &.selected {
      .field_overlay {
        background-color: rgba(89, 96, 178, 0.3);

        .field_icons {
          opacity: 0;
        }
      }
    }

    &:hover {
      .field-container {
        // background-color: #ffffff;
      }
    }
  }
}

.theme-dark {
  .field_list {
    &:hover {
      box-shadow: 0px 4px 15px rgb(255, 255, 255, 0.1);

      .field_overlay {
        background-color: #3f4143;
        opacity: 1;
      }
    }
  }
}

.theme-light {
  .field_list {
    &:hover {
      box-shadow: 0px 4px 15px rgba(0, 0, 0, 0.17);

      .field_overlay {
        background-color: #e8e8e8;
        opacity: 1;
      }
    }
  }
}

.q-item {
  min-height: 1.3rem;
  padding: 5px 10px;

  &__label {
    font-size: 0.75rem;
  }

  &.q-manual-focusable--focused > .q-focus-helper {
    background: none !important;
    opacity: 0.3 !important;
  }

  &.q-manual-focusable--focused > .q-focus-helper,
  &--active {
    // background-color: $selected-list-bg !important;
  }

  &.q-manual-focusable--focused > .q-focus-helper,
  &:hover,
  &--active {
    color: $primary;
  }
}

.q-field--dense .q-field__before,
.q-field--dense .q-field__prepend {
  padding: 0px 0px 0px 0px;
  height: auto;
  line-height: auto;
}

.q-field__native,
.q-field__input {
  padding: 0px 0px 0px 0px;
}

.q-field--dense .q-field__label {
  top: 5px;
}

.q-field--dense .q-field__control,
.q-field--dense .q-field__marginal {
  height: 34px;
}
</style>

<style lang="scss" scoped>
.index-table {
  .q-table {
    width: 100%;
    table-layout: fixed;

    .q-expansion-item {
      .q-item {
        display: flex;
        align-items: center;
        padding: 0;
        height: 25px !important;
        min-height: 25px !important;
      }

      .q-item__section--avatar {
        min-width: 12px;
        max-width: 12px;
        margin-right: 8px;
      }

      .filter-values-container {
        .q-item {
          padding-left: 4px;

          .q-focus-helper {
            background: none !important;
          }
        }
      }

      .q-item-type {
        &:hover {
          .field_overlay {
            visibility: visible;

            .q-icon {
              opacity: 1;
            }
          }
        }
      }

      .field-expansion-icon {
        img {
          width: 12px;
          height: 12px;
        }
      }
    }

    .field-container {
      &:hover {
        .field_overlay {
          visibility: visible;

          .q-icon {
            opacity: 1;
          }
        }
      }
    }

    .field_list {
      &.selected {
        .q-expansion-item {
          background-color: rgba(89, 96, 178, 0.3);
        }

        .field_overlay {
          // background-color: #ffffff;
        }
      }
    }
  }
}
</style>
