// Test application capability only; excluded from the SDK package.
import type {TurboModule} from 'react-native';
import {TurboModuleRegistry} from 'react-native';
export interface Spec extends TurboModule {
  prepareForeign(): Promise<void>;
  startForeign(): Promise<void>;
  releaseForeign(): Promise<void>;
  closeForeign(): Promise<void>;
}
export default TurboModuleRegistry.getEnforcing<Spec>('HoyaConsumerControl');
