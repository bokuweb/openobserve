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

<!-- eslint-disable vue/v-on-event-hyphenation -->
<!-- eslint-disable vue/attribute-hyphenation -->
<template>
  <q-page :key="store.state.selectedOrganization.identifier">
    <div ref="fullscreenDiv" :class="`${isFullscreen ? 'fullscreen' : ''}  ${store.state.theme === 'light' ? 'bg-white' : 'dark-mode'}`">
      <div
        :class="`${
          store.state.theme === 'light' ? 'bg-white' : 'dark-mode'
        } stickyHeader ${isFullscreen ? 'fullscreenHeader' : ''}`"
      >
        <div class="flex justify-between items-center q-pa-xs">
          <div class="flex">
            <q-btn
              v-if="!isFullscreen"
              no-caps
              @click="goBackToDashboardList"
              padding="xs"
              outline
              icon="arrow_back_ios_new"
            />
            <span class="q-table__title q-mx-md q-mt-xs">{{
              currentDashboardData.data.title
            }}</span>
          </div>
          <div class="flex">
            <q-btn
              v-if="!isFullscreen"
              outline
              class="dashboard-icons q-px-sm"
              size="sm"
              no-caps
              icon="add"
              @click="addPanelData"
              data-test="dashboard-panel-add"
            >
              <q-tooltip>{{ t("panel.add") }}</q-tooltip>
            </q-btn>
            <!-- <DateTimePicker 
            class="q-ml-sm"
            ref="refDateTime"
            v-model="selectedDate"
          /> -->
            <DateTimePickerDashboard
              ref="dateTimePicker"
              class="dashboard-icons q-ml-sm"
              size="sm"
              v-model="selectedDate"
            />
            <AutoRefreshInterval
              v-model="refreshInterval"
              trigger
              @trigger="refreshData"
              class="dashboard-icons"
              size="sm"
            />
            <q-btn
              outline
              class="dashboard-icons q-px-sm q-ml-sm"
              size="sm"
              no-caps
              icon="refresh"
              @click="refreshData"
            >
              <q-tooltip>{{ t("dashboard.refresh") }}</q-tooltip>
            </q-btn>
            <ExportDashboard
              v-if="!isFullscreen"
              :dashboardId="currentDashboardData.data?.dashboardId"
            />
            <q-btn
              v-if="!isFullscreen"
              outline
              class="dashboard-icons q-px-sm q-ml-sm"
              size="sm"
              no-caps
              icon="share"
              @click="shareLink"
              ><q-tooltip>{{ t("dashboard.share") }}</q-tooltip></q-btn
            >
            <q-btn
              v-if="!isFullscreen"
              outline
              class="dashboard-icons q-px-sm q-ml-sm"
              size="sm"
              no-caps
              icon="settings"
              @click="openSettingsDialog"
            >
              <q-tooltip>{{ t("dashboard.setting") }}</q-tooltip>
            </q-btn>
            <q-btn
              outline
              class="dashboard-icons q-px-sm q-ml-sm"
              size="sm"
              no-caps
              :icon="isFullscreen ? 'fullscreen_exit' : 'fullscreen'"
              @click="toggleFullscreen"
              ><q-tooltip>{{isFullscreen ? t("dashboard.exitFullscreen") : t("dashboard.fullscreen") }}</q-tooltip></q-btn
            >
          </div>
        </div>
        <q-separator></q-separator>
      </div>

      <RenderDashboardCharts
        @variablesData="variablesDataUpdated"
        :initialVariableValues="initialVariableValues"
        :viewOnly="false"
        :dashboardData="currentDashboardData.data"
        :currentTimeObj="currentTimeObj"
        :selectedDateForViewPanel="selectedDate"
        @onDeletePanel="onDeletePanel"
        @updated:data-zoom="onDataZoom"
      />

      <q-dialog
        v-model="showDashboardSettingsDialog"
        position="right"
        full-height
        maximized
      >
        <DashboardSettings @refresh="loadDashboard" />
      </q-dialog>
    </div>
  </q-page>
</template>

<script lang="ts">
// @ts-nocheck
import { defineComponent, ref, watch, onActivated, nextTick } from "vue";
import { useStore } from "vuex";
import { useI18n } from "vue-i18n";
import DateTimePicker from "../../components/DateTimePicker.vue";
import DateTimePickerDashboard from "@/components/DateTimePickerDashboard.vue";
import { useRouter } from "vue-router";
import { getConsumableDateTime, getDashboard } from "../../utils/commons.ts";
import { parseDuration, generateDurationLabel } from "../../utils/date";
import { toRaw, unref, reactive } from "vue";
import { useRoute } from "vue-router";
import { deletePanel } from "../../utils/commons";
import AutoRefreshInterval from "../../components/AutoRefreshInterval.vue";
import ExportDashboard from "../../components/dashboards/ExportDashboard.vue";
import DashboardSettings from "./DashboardSettings.vue";
import RenderDashboardCharts from "./RenderDashboardCharts.vue";
import { copyToClipboard, useQuasar } from "quasar";
import { onMounted } from "vue";
import { onUnmounted } from "vue";

export default defineComponent({
  name: "ViewDashboard",
  emits: ["onDeletePanel"],
  components: {
    DateTimePickerDashboard,
    AutoRefreshInterval,
    ExportDashboard,
    DashboardSettings,
    RenderDashboardCharts,
  },
  setup() {
    const { t } = useI18n();
    const route = useRoute();
    const router = useRouter();
    const store = useStore();
    const $q = useQuasar();
    const currentDashboardData = reactive({
      data: {},
    });

    // boolean to show/hide settings sidebar
    const showDashboardSettingsDialog = ref(false);

    // variables data
    const variablesData = reactive({});
    const variablesDataUpdated = (data: any) => {
      Object.assign(variablesData, data);
      const variableObj = {};
      data.values.forEach((variable) => {
        if (variable.type === "dynamic_filters") {
          const filters = (variable.value || []).filter(
            (item: any) => item.name && item.operator && item.value
          );
          const encodedFilters = filters.map((item: any) => ({
            name: item.name,
            operator: item.operator,
            value: item.value,
          }));
          variableObj[`var-${variable.name}`] = encodeURIComponent(
            JSON.stringify(encodedFilters)
          );
        } else {
          variableObj[`var-${variable.name}`] = variable.value;
        }
      });
      router.replace({
        query: {
          org_identifier: store.state.selectedOrganization.identifier,
          dashboard: route.query.dashboard,
          folder: route.query.folder,
          refresh: generateDurationLabel(refreshInterval.value),
          ...getQueryParamsForDuration(selectedDate.value),
          ...variableObj,
        },
      });
    };

    // ======= [START] default variable values

    const initialVariableValues = {};
    Object.keys(route.query).forEach((key) => {
      if (key.startsWith("var-")) {
        const newKey = key.slice(4);
        initialVariableValues[newKey] = route.query[key];
      }
    });
    // ======= [END] default variable values

    onActivated(async () => {
      await loadDashboard();
    });

    const loadDashboard = async () => {
      currentDashboardData.data = await getDashboard(
        store,
        route.query.dashboard,
        route.query.folder ?? "default"
      );
      
      // if variables data is null, set it to empty list
      if (
        !(
          currentDashboardData.data?.variables &&
          currentDashboardData.data?.variables?.list.length
        )
      ) {
        variablesData.isVariablesLoading = false;
        variablesData.values = [];
      }
    };

    const openSettingsDialog = () => {
      showDashboardSettingsDialog.value = true;
    };

    // [START] date picker related variables --------

    /**
     * Retrieves the selected date from the query parameters.
     */
    const getSelectedDateFromQueryParams = (params) => ({
      valueType: params.period
        ? "relative"
        : params.from && params.to
        ? "absolute"
        : "relative",
      startTime: params.from ? params.from : null,
      endTime: params.to ? params.to : null,
      relativeTimePeriod: params.period ? params.period : null,
    });

    const dateTimePicker = ref(null); // holds a reference to the date time picker

    // holds the date picker v-modal
    const selectedDate = ref(getSelectedDateFromQueryParams(route.query));

    // holds the current time for the dashboard
    const currentTimeObj = ref({});

    // refresh interval v-model
    const refreshInterval = ref(0);

    // when the date changes from the picker, update the current time object for the dashboard
    watch(selectedDate, () => {
      currentTimeObj.value = {
        start_time: new Date(selectedDate.value.startTime),
        end_time: new Date(selectedDate.value.endTime),
      };
    });

    const getQueryParamsForDuration = (data: any) => {
      if (data.relativeTimePeriod) {
        return {
          period: data.relativeTimePeriod,
        };
      } else {
        return {
          from: data.startTime,
          to: data.endTime,
        };
      }
    };

    // [END] date picker related variables

    // back button to render dashboard List page
    const goBackToDashboardList = () => {
      return router.push({
        path: "/dashboards",
        query: {
          dashboard: route.query.dashboard,
          folder: route.query.folder ?? "default",
        },
      });
    };

    //add panel
    const addPanelData = () => {
      return router.push({
        path: "/dashboards/add_panel",
        query: {
          dashboard: route.query.dashboard,
          folder: route.query.folder ?? "default",
        },
      });
    };

    const refreshData = () => {
      dateTimePicker.value.refresh();
    };

    const onDataZoom = (event: any) => {
      const selectedDateObj = {
        start: new Date(event.start),
        end: new Date(event.end),
      };
      // Truncate seconds and milliseconds from the dates
      selectedDateObj.start.setSeconds(0, 0);
      selectedDateObj.end.setSeconds(0, 0);

      // Compare the truncated dates
      if (selectedDateObj.start.getTime() === selectedDateObj.end.getTime()) {
        // Increment the end date by 1 minute
        selectedDateObj.end.setMinutes(selectedDateObj.end.getMinutes() + 1);
      }

      // set it as a absolute time
      dateTimePicker?.value?.setCustomDate("absolute", selectedDateObj);
    };

    // ------- work with query params ----------
    onActivated(async () => {
      const params = route.query;

      if (params.refresh) {
        refreshInterval.value = parseDuration(params.refresh);
      }

      // This is removed due to the bug of the new date time component
      // and is now rendered when the setup method is called
      // instead of onActivated
      // if (params.period || (params.to && params.from)) {
      //   selectedDate.value = getSelectedDateFromQueryParams(params);
      // }

      // resize charts if needed
      await nextTick();
      window.dispatchEvent(new Event("resize"));
    });

    // whenever the refreshInterval is changed, update the query params
    watch([refreshInterval, selectedDate], () => {
      router.replace({
        query: {
          org_identifier: store.state.selectedOrganization.identifier,
          dashboard: route.query.dashboard,
          folder: route.query.folder,
          refresh: generateDurationLabel(refreshInterval.value),
          ...getQueryParamsForDuration(selectedDate.value),
        },
      });
    });

    const onDeletePanel = async (panelId: any) => {
      await deletePanel(
        store,
        route.query.dashboard,
        panelId,
        route.query.folder ?? "default"
      );
      await loadDashboard();
    };

    const shareLink = () => {
      const urlObj = new URL(window.location.href);
      const urlSearchParams = urlObj?.searchParams;

      // if relative time period, convert to absolute time
      if (urlSearchParams?.has("period")) {
        urlSearchParams.delete("period");
        urlSearchParams.set(
          "from",
          currentTimeObj?.value?.start_time?.getTime()
        );
        urlSearchParams.set("to", currentTimeObj?.value?.end_time?.getTime());
      }

      copyToClipboard(urlObj?.href)
        .then(() => {
          $q.notify({
            type: "positive",
            message: "Link Copied Successfully!",
            timeout: 5000,
          });
        })
        .catch(() => {
          $q.notify({
            type: "negative",
            message: "Error while copy link.",
            timeout: 5000,
          });
        });
    };

    // Fullscreen
    const fullscreenDiv = ref(null);
    const isFullscreen = ref(false);

    const toggleFullscreen = () => {
      if (!document.fullscreenElement) {
        if (fullscreenDiv.value.requestFullscreen) {
          fullscreenDiv.value.requestFullscreen();
        }
        isFullscreen.value = true;
        document.body.style.overflow = "hidden"; // Disable body scroll
      } else {
        if (document.exitFullscreen) {
          document.exitFullscreen();
        }
        isFullscreen.value = false;
        document.body.style.overflow = ""; // Enable body scroll
      }
    };

    const onFullscreenChange = () => {
      if (!document.fullscreenElement) {
        isFullscreen.value = false;
      }
    };

    onMounted(() => {
      document.addEventListener("fullscreenchange", onFullscreenChange);
    });

    onUnmounted(() => {
      document.removeEventListener("fullscreenchange", onFullscreenChange);
    });

    onActivated(() => {
      isFullscreen.value = false;
    });

    return {
      currentDashboardData,
      toggleFullscreen,
      fullscreenDiv,
      isFullscreen,
      goBackToDashboardList,
      addPanelData,
      t,
      getDashboard,
      store,
      // date variables
      dateTimePicker,
      selectedDate,
      currentTimeObj,
      refreshInterval,
      // ----------------
      refreshData,
      onDeletePanel,
      variablesData,
      variablesDataUpdated,
      showDashboardSettingsDialog,
      openSettingsDialog,
      loadDashboard,
      initialVariableValues,
      getQueryParamsForDuration,
      onDataZoom,
      shareLink,
    };
  },
});
</script>

<style lang="scss" scoped>
.q-table {
  &__top {
    border-bottom: 1px solid $border-color;
    justify-content: flex-end;
  }
}

.dark-mode {
  background-color: $dark-page;
}

.bg-white {
  background-color: $white;
}

.stickyHeader {
  position: sticky;
  top: 57px;
  z-index: 1001;
}
.stickyHeader.fullscreenHeader {
  top: 0px;
}

.fullscreen {
  width: 100vw;
  height: 100vh;
  overflow-y: auto; /* Enables scrolling within the div */
  position: fixed;
  top: 0;
  left: 0;
  z-index: 10000; /* Ensure it's on top */
  /* Additional styling as needed */
}

.dashboard-icons {
  height: 30px;
}
</style>
