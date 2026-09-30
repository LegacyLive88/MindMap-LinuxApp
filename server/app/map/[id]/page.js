"use client";

import { useParams } from "next/navigation";
import MapEditor from "../../components/MapEditor";

export default function MapPage() {
  const params = useParams();
  const canvasId = Array.isArray(params.id) ? params.id[0] : params.id;
  if (!canvasId) return null;
  return <MapEditor canvasId={canvasId} />;
}
