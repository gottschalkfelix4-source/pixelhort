// Google Cast (Web Sender SDK) – loaded lazily, degrades gracefully when absent.
import { api, type AssetDto } from './api';

const SDK_URL = 'https://www.gstatic.com/cv/js/sender/v1/cast_sender.js?loadCastFramework=1';

/* eslint-disable @typescript-eslint/no-explicit-any */
declare global {
  interface Window {
    __onGCastApiAvailable?: (available: boolean) => void;
    cast?: any;
    chrome?: any;
  }
}

let loader: Promise<boolean> | null = null;

export function loadCast(): Promise<boolean> {
  if (loader) return loader;
  loader = new Promise<boolean>((resolve) => {
    // The Cast sender only works in Chromium based browsers.
    const isChromium = /Chrome|CriOS|Edg\//.test(navigator.userAgent) && !/Firefox/.test(navigator.userAgent);
    if (!isChromium) return resolve(false);
    const timer = setTimeout(() => resolve(false), 10000);
    window.__onGCastApiAvailable = (available: boolean) => {
      clearTimeout(timer);
      if (!available || !window.cast?.framework || !window.chrome?.cast) return resolve(false);
      try {
        window.cast.framework.CastContext.getInstance().setOptions({
          receiverApplicationId: window.chrome.cast.media.DEFAULT_MEDIA_RECEIVER_APP_ID,
          autoJoinPolicy: window.chrome.cast.AutoJoinPolicy.ORIGIN_SCOPED,
        });
        resolve(true);
      } catch {
        resolve(false);
      }
    };
    const s = document.createElement('script');
    s.src = SDK_URL;
    s.async = true;
    s.onerror = () => {
      clearTimeout(timer);
      resolve(false);
    };
    document.head.appendChild(s);
  });
  loader.then((ok) => {
    if (!ok) loader = null; // allow retry later
  });
  return loader;
}

export class CastUnavailableError extends Error {}

/** Casts the asset to a (user selected) Cast device. Returns the device name. */
export async function castAsset(asset: AssetDto): Promise<string> {
  const ok = await loadCast();
  if (!ok) throw new CastUnavailableError('cast unavailable');
  const ctx = window.cast.framework.CastContext.getInstance();
  let session = ctx.getCurrentSession();
  if (!session) {
    await ctx.requestSession();
    session = ctx.getCurrentSession();
  }
  if (!session) throw new Error('no session');
  const urls = await api.castUrl(asset.id);
  const chromeCast = window.chrome.cast;
  const isVideo = asset.type === 'VIDEO';
  const info = new chromeCast.media.MediaInfo(isVideo ? urls.url : urls.imageUrl, isVideo ? urls.mimeType || 'video/mp4' : 'image/jpeg');
  const meta = isVideo ? new chromeCast.media.GenericMediaMetadata() : new chromeCast.media.PhotoMediaMetadata();
  meta.title = asset.originalFileName;
  info.metadata = meta;
  info.streamType = chromeCast.media.StreamType.BUFFERED;
  const req = new chromeCast.media.LoadRequest(info);
  req.autoplay = true;
  await session.loadMedia(req);
  return session.getCastDevice()?.friendlyName ?? 'Chromecast';
}
