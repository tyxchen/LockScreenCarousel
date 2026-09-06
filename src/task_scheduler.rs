use windows::core::*;
use windows::Win32::{Foundation::VARIANT_TRUE, Security::Authentication::Identity::*, System::Com::*, System::TaskScheduler::*};
use windows_time::DateTime;

use crate::{CarouselTrigger, b};

fn get_user_name() -> Result<BSTR> {
    unsafe {
        let mut user_name_len = 0;
        let _ = GetUserNameExW(NameSamCompatible, None, &mut user_name_len);
        let mut buffer = vec![0u16; user_name_len as usize];
        let user_name = PWSTR::from_raw(buffer.as_mut_ptr());
        if GetUserNameExW(NameSamCompatible, Some(user_name), &mut user_name_len) {
            Ok(BSTR::from_wide(user_name.as_wide()))
        } else {
            Err(Error::from_thread())
        }
    }
}

fn create_unlock_trigger(triggers: &ITriggerCollection) -> Result<ISessionStateChangeTrigger> {
    unsafe {
        let trigger: ISessionStateChangeTrigger = triggers.Create(TASK_TRIGGER_SESSION_STATE_CHANGE)?.cast()?;

        trigger.SetId(&b!("Unlock trigger"))?;
        trigger.SetStateChange(TASK_SESSION_UNLOCK)?;

        Ok(trigger)
    }
}

fn create_schedule_trigger(triggers: &ITriggerCollection, interval: u32) -> Result<ITimeTrigger> {
    unsafe {
        let trigger: ITimeTrigger = triggers.Create(TASK_TRIGGER_TIME)?.cast()?;

        let cur_time = DateTime::now();
        let cur_time_str = format!("{}", cur_time);

        trigger.SetId(&b!("Time trigger"))?;
        trigger.SetStartBoundary(&BSTR::from(cur_time_str))?;

        let interval_str = format!("PT{}M", interval);
        let repetition = trigger.Repetition()?;
        repetition.SetInterval(&BSTR::from(interval_str))?;

        Ok(trigger)
    }
}

pub fn schedule_task(trigger_type: CarouselTrigger, interval: u32) -> Result<()> {
    let task_name = unsafe { b!("Lock Screen Carousel") };
    let task_path = {
        let mut path = std::env::current_exe()?.parent().unwrap().to_path_buf();
        path.push("task.exe");
        let exec_string = path.into_string().unwrap();
        BSTR::from(exec_string)
    };
    let user_name = get_user_name()?;

    unsafe {
        let task_service: ITaskService = CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
        task_service.Connect(&Default::default(), &Default::default(), &Default::default(), &Default::default())?;

        let task_folder = task_service.GetFolder(&b!("\\"))?;
        if let Ok(task) = task_folder.GetTask(&task_name) {
            task.Stop(0).and_then(|_| task_folder.DeleteTask(&task_name, 0))?;
        }

        if trigger_type == CarouselTrigger::Never {
            return Ok(());
        }

        let new_task = task_service.NewTask(0)?;

        let settings = new_task.Settings()?;
        settings.SetStartWhenAvailable(VARIANT_TRUE)?;

        let triggers = new_task.Triggers()?;

        if trigger_type == CarouselTrigger::Lock {
            let trigger = create_unlock_trigger(&triggers)?;
            trigger.SetUserId(&user_name)?;
        } else if trigger_type == CarouselTrigger::Interval {
            create_schedule_trigger(&triggers, interval)?;
        }

        let actions = new_task.Actions()?;
        let exec_action: IExecAction = actions.Create(TASK_ACTION_EXEC)?.cast()?;
        exec_action.SetPath(&task_path)?;

        let _ = task_folder.RegisterTaskDefinition(
            &task_name,
            &new_task,
            TASK_CREATE_OR_UPDATE.0,
            &Default::default(),
            &Default::default(),
            TASK_LOGON_INTERACTIVE_TOKEN,
            &Default::default()
        )?;
    }

    Ok(())
}
