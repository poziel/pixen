<script setup lang="ts">
import {helpParts} from './toolInfo';
defineProps<{text:string}>();
</script>
<template><template v-for="(part,index) in helpParts(text)" :key="index"><code v-if="part.key">{{part.text}}</code><template v-else>{{part.text}}</template></template></template>
